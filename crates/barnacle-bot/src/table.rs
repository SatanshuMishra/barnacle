use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use barnacle_guess::Draw;
use barnacle_guess::GameError;
use barnacle_guess::Guess;
use barnacle_guess::Hint;
use barnacle_guess::RecentShips;
use barnacle_guess::Reveal;
use barnacle_guess::Round;
use barnacle_guess::RoundOptions;
use barnacle_guess::ShipBook;
use barnacle_guess::Snowflake;
use barnacle_guess::Solve;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use rand::rngs::StdRng;
use tokio::sync::Mutex;

use crate::ids::ChannelId;
use crate::ids::Place;
use crate::solves::SolveRecord;
use crate::solves::SolveStore;

#[derive(Debug, thiserror::Error)]
#[error("the Discord call failed")]
pub struct AnnounceError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
    Solved {
        solve: Solve,
        reveal: Reveal,
        message: Snowflake,
        personal_best: Option<bool>,
    },
    TimedOut {
        reveal: Reveal,
    },
    Cancelled {
        by: UserId,
        reveal: Reveal,
    },
}

pub trait Announcer: Send + Sync + 'static {
    fn post_hint(
        &self,
        channel: ChannelId,
        hint: &Hint,
    ) -> impl Future<Output = Result<(), AnnounceError>> + Send;

    fn post_ending(
        &self,
        channel: ChannelId,
        round_post: Snowflake,
        ending: &Ending,
    ) -> impl Future<Output = Result<(), AnnounceError>> + Send;
}

#[derive(Debug)]
pub enum StartOutcome<E> {
    Started { number: u64 },
    Busy,
    NoShips(GameError),
    PostFailed(E),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    Cancelled,
    Refused,
    AlreadyOver,
}

struct Active {
    number: u64,
    round: Round,
}

#[derive(Default)]
struct Seat {
    recent: RecentShips,
    active: Option<Active>,
}

pub struct Table<A, S> {
    book: ShipBook,
    solves: S,
    announcer: A,
    timing: Timing,
    rng: std::sync::Mutex<StdRng>,
    seats: std::sync::Mutex<HashMap<ChannelId, Arc<Mutex<Seat>>>>,
    next_number: AtomicU64,
}

impl<A: Announcer, S: SolveStore> Table<A, S> {
    pub fn new(book: ShipBook, solves: S, announcer: A, timing: Timing, rng: StdRng) -> Arc<Self> {
        Arc::new(Self {
            book,
            solves,
            announcer,
            timing,
            rng: std::sync::Mutex::new(rng),
            seats: std::sync::Mutex::new(HashMap::new()),
            next_number: AtomicU64::new(1),
        })
    }

    pub fn book(&self) -> &ShipBook {
        &self.book
    }

    pub fn solves(&self) -> &S {
        &self.solves
    }

    pub async fn start<P, F, E>(
        self: &Arc<Self>,
        place: Place,
        options: RoundOptions,
        invoker: UserId,
        post: P,
    ) -> StartOutcome<E>
    where
        P: FnOnce(Draw, u64) -> F,
        F: Future<Output = Result<Snowflake, E>>,
    {
        let seat = self.seat(place.channel);
        let mut seat = seat.lock().await;
        if seat.active.is_some() {
            return StartOutcome::Busy;
        }
        let draw = match self.draw(&options, &seat.recent) {
            Ok(draw) => draw,
            Err(error) => return StartOutcome::NoShips(error),
        };
        let number = self.next_number.fetch_add(1, Ordering::Relaxed);
        let posted = match post(draw.clone(), number).await {
            Ok(posted) => posted,
            Err(error) => return StartOutcome::PostFailed(error),
        };
        *seat = Seat {
            recent: seat.recent.remember(draw.ship().clone()),
            active: Some(Active {
                number,
                round: draw.start(invoker, posted),
            }),
        };
        drop(seat);
        tracing::info!(channel = place.channel.get(), number, "round started");
        self.spawn_timer(place, number);
        StartOutcome::Started { number }
    }

    pub async fn hear(&self, place: Place, guess: Guess<'_>) {
        if guess.author_is_bot {
            return;
        }
        let Some(seat) = self.existing_seat(place.channel) else {
            return;
        };
        let mut seat = seat.lock().await;
        let solved = seat
            .active
            .as_ref()
            .and_then(|active| active.round.judge(&guess));
        let Some(solve) = solved else {
            return;
        };
        let Some(active) = seat.active.take() else {
            return;
        };
        let record = SolveRecord {
            guild: place.guild,
            user: solve.winner,
            ship: solve.ship.clone(),
            elapsed: solve.elapsed,
            solved_at_ms: guess.message.unix_millis(),
        };
        let personal_best = match self.solves.record(&record).await {
            Ok(best) => Some(best),
            Err(error) => {
                tracing::error!(%error, "a solve could not be recorded");
                None
            }
        };
        let ending = Ending::Solved {
            reveal: active.round.draw().reveal().clone(),
            solve,
            message: guess.message,
            personal_best,
        };
        self.announce_ending(place.channel, &active, &ending).await;
    }

    pub async fn cancel(
        &self,
        place: Place,
        number: u64,
        user: UserId,
        can_manage_messages: bool,
    ) -> CancelOutcome {
        let Some(seat) = self.existing_seat(place.channel) else {
            return CancelOutcome::AlreadyOver;
        };
        let mut seat = seat.lock().await;
        let allowed = match &seat.active {
            Some(active) if active.number == number => {
                active.round.may_cancel(user, can_manage_messages)
            }
            Some(_) | None => return CancelOutcome::AlreadyOver,
        };
        if !allowed {
            return CancelOutcome::Refused;
        }
        let Some(active) = seat.active.take() else {
            return CancelOutcome::AlreadyOver;
        };
        let ending = Ending::Cancelled {
            by: user,
            reveal: active.round.draw().reveal().clone(),
        };
        self.announce_ending(place.channel, &active, &ending).await;
        CancelOutcome::Cancelled
    }

    fn draw(&self, options: &RoundOptions, recent: &RecentShips) -> Result<Draw, GameError> {
        let mut rng = self
            .rng
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.book.draw(options, recent, &mut *rng)
    }

    fn seat(&self, channel: ChannelId) -> Arc<Mutex<Seat>> {
        let mut seats = self
            .seats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(seats.entry(channel).or_default())
    }

    fn existing_seat(&self, channel: ChannelId) -> Option<Arc<Mutex<Seat>>> {
        self.seats
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&channel)
            .map(Arc::clone)
    }

    fn spawn_timer(self: &Arc<Self>, place: Place, number: u64) {
        let table = Arc::clone(self);
        tokio::spawn(async move { table.run_timer(place.channel, number).await });
    }

    async fn run_timer(&self, channel: ChannelId, number: u64) {
        tokio::time::sleep(self.timing.before_hint).await;
        if !self.post_hint(channel, number).await {
            return;
        }
        tokio::time::sleep(self.timing.after_hint).await;
        self.time_out(channel, number).await;
    }

    async fn post_hint(&self, channel: ChannelId, number: u64) -> bool {
        let Some(seat) = self.existing_seat(channel) else {
            return false;
        };
        let seat = seat.lock().await;
        let Some(active) = seat
            .active
            .as_ref()
            .filter(|active| active.number == number)
        else {
            return false;
        };
        if let Err(error) = self
            .announcer
            .post_hint(channel, active.round.draw().hint())
            .await
        {
            tracing::error!(%error, "a hint could not be posted");
        }
        true
    }

    async fn time_out(&self, channel: ChannelId, number: u64) {
        let Some(seat) = self.existing_seat(channel) else {
            return;
        };
        let mut seat = seat.lock().await;
        if seat
            .active
            .as_ref()
            .is_none_or(|active| active.number != number)
        {
            return;
        }
        let Some(active) = seat.active.take() else {
            return;
        };
        let ending = Ending::TimedOut {
            reveal: active.round.draw().reveal().clone(),
        };
        self.announce_ending(channel, &active, &ending).await;
    }

    async fn announce_ending(&self, channel: ChannelId, active: &Active, ending: &Ending) {
        tracing::info!(
            channel = channel.get(),
            number = active.number,
            "round ended"
        );
        if let Err(error) = self
            .announcer
            .post_ending(channel, active.round.posted(), ending)
            .await
        {
            tracing::error!(%error, "a round ending could not be posted");
        }
    }
}

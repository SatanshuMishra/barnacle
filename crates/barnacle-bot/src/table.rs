use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use barnacle_guess::Draw;
use barnacle_guess::GameError;
use barnacle_guess::Guess;
use barnacle_guess::Hint;
use barnacle_guess::Quiet;
use barnacle_guess::RecentShips;
use barnacle_guess::Reveal;
use barnacle_guess::Round;
use barnacle_guess::RoundOptions;
use barnacle_guess::SeriesLength;
use barnacle_guess::ShipBook;
use barnacle_guess::Snowflake;
use barnacle_guess::Solve;
use barnacle_guess::Tally;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use rand::rngs::StdRng;
use tokio::sync::Mutex;

use crate::failure;
use crate::failure::Failure;
use crate::failure::Scope;
use crate::ids::ChannelId;
use crate::ids::Place;
use crate::solves::SolveRecord;
use crate::solves::SolveStore;
use crate::text;

pub const ANNOUNCE_TIMEOUT: Duration = Duration::from_secs(5);
pub const SETTLE_WINDOW: Duration = Duration::from_millis(250);
pub const COUNTDOWN_SECONDS: u32 = 5;
pub const CHECK_WINDOW: Duration = Duration::from_secs(10);
const COUNTDOWN_EDITS: [u32; 3] = [3, 2, 1];
const SERIES_COMPLETED: &str = "a silhouette series completed";
const SERIES_ENDED_BY_PLAYER: &str = "a silhouette series was ended by a player";
const SERIES_WENT_QUIET: &str = "a silhouette series ended after nobody answered";
const SERIES_UNPOSTED: &str = "a silhouette series ended after a round could not be posted";

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

impl Ending {
    pub fn winning_message(&self) -> Option<Snowflake> {
        match self {
            Ending::Solved { message, .. } => Some(*message),
            Ending::TimedOut { .. } | Ending::Cancelled { .. } => None,
        }
    }
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

    fn post_series_round(
        &self,
        channel: ChannelId,
        draw: &Draw,
        number: u64,
        round: u32,
        length: SeriesLength,
    ) -> impl Future<Output = Result<Snowflake, AnnounceError>> + Send;

    fn post_series_ending(
        &self,
        channel: ChannelId,
        round_post: Snowflake,
        content: &str,
        reply_to: Option<Snowflake>,
        still_playing: Option<u64>,
    ) -> impl Future<Output = Result<Snowflake, AnnounceError>> + Send;

    fn edit_series_message(
        &self,
        channel: ChannelId,
        message: Snowflake,
        content: &str,
    ) -> impl Future<Output = Result<(), AnnounceError>> + Send;
}

#[derive(Debug)]
pub enum StartOutcome<E> {
    Started { number: u64 },
    Busy,
    NoShips(GameError),
    PostFailed(E),
    PostTimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    Cancelled,
    Refused,
    AlreadyOver,
}

struct Leader {
    solve: Solve,
    message: Snowflake,
}

#[derive(Debug, Clone, Copy)]
struct SeriesRound {
    position: u32,
    heard: bool,
    ends_series: bool,
}

impl SeriesRound {
    fn at(position: u32) -> Self {
        Self {
            position,
            heard: false,
            ends_series: false,
        }
    }

    fn heard(self) -> Self {
        Self {
            heard: true,
            ..self
        }
    }

    fn ending_series(self) -> Self {
        Self {
            ends_series: true,
            ..self
        }
    }
}

struct Active {
    number: u64,
    round: Round,
    leader: Option<Leader>,
    series: Option<SeriesRound>,
}

struct Check {
    message: Option<Snowflake>,
    result: String,
}

struct Series {
    number: u64,
    starter: UserId,
    options: RoundOptions,
    length: SeriesLength,
    played: u32,
    tally: Tally,
    quiet: Quiet,
    check: Option<Check>,
}

impl Series {
    fn after(self, heard: bool, ending: &Ending) -> Self {
        let tally = match ending {
            Ending::Solved { solve, .. } => self.tally.won(solve.winner, solve.elapsed),
            Ending::TimedOut { .. } | Ending::Cancelled { .. } => self.tally,
        };
        Self {
            tally,
            quiet: self.quiet.after_round(heard),
            ..self
        }
    }

    fn standings(&self) -> String {
        text::series_standings(&self.tally, self.played, self.length)
    }

    fn overview(&self) -> String {
        text::series_overview(&self.tally, self.played, self.length)
    }
}

#[derive(Default)]
struct Seat {
    recent: RecentShips,
    active: Option<Active>,
    series: Option<Series>,
}

impl Seat {
    fn busy(&self) -> bool {
        self.active.is_some() || self.series.is_some()
    }
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
        if seat.busy() {
            return StartOutcome::Busy;
        }
        self.open_round(place, &mut seat, &options, invoker, None, |draw, number| {
            command_post(post(draw, number))
        })
        .await
    }

    pub async fn start_series<P, F, E>(
        self: &Arc<Self>,
        place: Place,
        options: RoundOptions,
        length: SeriesLength,
        invoker: UserId,
        post_intro: P,
    ) -> StartOutcome<E>
    where
        P: FnOnce() -> F,
        F: Future<Output = Result<Snowflake, E>>,
    {
        let seat = self.seat(place.channel);
        let mut seat = seat.lock().await;
        if seat.busy() {
            return StartOutcome::Busy;
        }
        if let Err(error) = self.draw(&options, &seat.recent) {
            return StartOutcome::NoShips(error);
        }
        let number = self.next_number.fetch_add(1, Ordering::Relaxed);
        let intro = match command_post(post_intro()).await {
            Ok(intro) => intro,
            Err(outcome) => return outcome,
        };
        seat.series = Some(Series {
            number,
            starter: invoker,
            options,
            length,
            played: 0,
            tally: Tally::default(),
            quiet: Quiet::default(),
            check: None,
        });
        drop(seat);
        failure::record(
            "guess.series.started",
            "a silhouette series started",
            &round_scope(place, number),
        );
        self.spawn_count_down(place, number, Some(intro), move |remaining| {
            text::series_intro(&options, length, remaining)
        });
        StartOutcome::Started { number }
    }

    pub async fn hear(self: &Arc<Self>, place: Place, guess: Guess<'_>) {
        if guess.author_is_bot {
            return;
        }
        let Some(seat) = self.existing_seat(place.channel) else {
            return;
        };
        let mut seat = seat.lock().await;
        let Some(active) = seat.active.as_mut() else {
            return;
        };
        if guess.message > active.round.posted() {
            active.series = active.series.map(SeriesRound::heard);
        }
        let Some(solve) = active.round.judge(&guess) else {
            return;
        };
        let first = active.leader.is_none();
        let earlier = active
            .leader
            .as_ref()
            .is_none_or(|leader| guess.message < leader.message);
        if earlier {
            active.leader = Some(Leader {
                solve,
                message: guess.message,
            });
        }
        let number = active.number;
        drop(seat);
        if first {
            let table = Arc::clone(self);
            tokio::spawn(async move { table.settle(place, number).await });
        }
    }

    async fn settle(self: &Arc<Self>, place: Place, number: u64) {
        tokio::time::sleep(SETTLE_WINDOW).await;
        let Some(seat) = self.existing_seat(place.channel) else {
            return;
        };
        let mut seat = seat.lock().await;
        let Some(active) = seat.active.take_if(|active| active.number == number) else {
            return;
        };
        let Some(leader) = &active.leader else {
            return;
        };
        let record = SolveRecord {
            guild: place.guild,
            user: leader.solve.winner,
            ship: leader.solve.ship.clone(),
            elapsed: leader.solve.elapsed,
            solved_at_ms: leader.message.unix_millis(),
        };
        let personal_best = match self.solves.record(&record).await {
            Ok(best) => Some(best),
            Err(error) => {
                failure::report(
                    "guess.solve.failed",
                    "a solve could not be saved",
                    &Failure::from_error(&error),
                    &round_scope(place, number).user(record.user),
                );
                None
            }
        };
        let ending = Ending::Solved {
            solve: leader.solve.clone(),
            reveal: active.round.draw().reveal().clone(),
            message: leader.message,
            personal_best,
        };
        self.hand_off(place, &mut seat, &active, &ending).await;
    }

    pub async fn cancel(
        self: &Arc<Self>,
        place: Place,
        number: u64,
        user: UserId,
        can_manage_messages: bool,
    ) -> CancelOutcome {
        self.press(
            place,
            number,
            user,
            can_manage_messages,
            std::convert::identity,
        )
        .await
    }

    pub async fn skip(
        self: &Arc<Self>,
        place: Place,
        number: u64,
        user: UserId,
        can_manage_messages: bool,
    ) -> CancelOutcome {
        self.press(place, number, user, can_manage_messages, SeriesRound::heard)
            .await
    }

    pub async fn end_series(
        self: &Arc<Self>,
        place: Place,
        number: u64,
        user: UserId,
        can_manage_messages: bool,
    ) -> CancelOutcome {
        self.press(
            place,
            number,
            user,
            can_manage_messages,
            SeriesRound::ending_series,
        )
        .await
    }

    pub async fn still_playing(self: &Arc<Self>, place: Place, series: u64) -> CancelOutcome {
        let Some(seat) = self.existing_seat(place.channel) else {
            return CancelOutcome::AlreadyOver;
        };
        let mut seat = seat.lock().await;
        let Some(check) = seat
            .series
            .as_mut()
            .filter(|current| current.number == series)
            .and_then(|current| current.check.take())
        else {
            return CancelOutcome::AlreadyOver;
        };
        let Some(resumed) = seat.series.take().map(|current| Series {
            quiet: Quiet::default(),
            ..current
        }) else {
            return CancelOutcome::AlreadyOver;
        };
        let content = countdown_content(
            check.result,
            resumed.standings(),
            resumed.played + 1,
            resumed.length,
        );
        seat.series = Some(resumed);
        drop(seat);
        if let Some(message) = check.message {
            self.spawn_edit(place, series, message, content(COUNTDOWN_SECONDS));
        }
        self.spawn_count_down(place, series, check.message, content);
        CancelOutcome::Cancelled
    }

    async fn press(
        self: &Arc<Self>,
        place: Place,
        number: u64,
        user: UserId,
        can_manage_messages: bool,
        mark: impl FnOnce(SeriesRound) -> SeriesRound,
    ) -> CancelOutcome {
        let Some(seat) = self.existing_seat(place.channel) else {
            return CancelOutcome::AlreadyOver;
        };
        let mut seat = seat.lock().await;
        let allowed = match &seat.active {
            Some(active) if active.number == number && active.leader.is_none() => {
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
        let active = Active {
            series: active.series.map(mark),
            ..active
        };
        let ending = Ending::Cancelled {
            by: user,
            reveal: active.round.draw().reveal().clone(),
        };
        self.hand_off(place, &mut seat, &active, &ending).await;
        CancelOutcome::Cancelled
    }

    async fn open_round<P, F, E>(
        self: &Arc<Self>,
        place: Place,
        seat: &mut Seat,
        options: &RoundOptions,
        invoker: UserId,
        series: Option<SeriesRound>,
        post: P,
    ) -> StartOutcome<E>
    where
        P: FnOnce(Draw, u64) -> F,
        F: Future<Output = Result<Snowflake, StartOutcome<E>>>,
    {
        let draw = match self.draw(options, &seat.recent) {
            Ok(draw) => draw,
            Err(error) => return StartOutcome::NoShips(error),
        };
        let number = self.next_number.fetch_add(1, Ordering::Relaxed);
        let posted = match post(draw.clone(), number).await {
            Ok(posted) => posted,
            Err(outcome) => return outcome,
        };
        seat.recent = seat.recent.remember(draw.ship().clone());
        seat.active = Some(Active {
            number,
            round: draw.start(invoker, posted),
            leader: None,
            series,
        });
        failure::record(
            "guess.round.started",
            "a silhouette round started",
            &round_scope(place, number),
        );
        self.spawn_timer(place, number);
        StartOutcome::Started { number }
    }

    async fn next_series_round(self: &Arc<Self>, place: Place, series: u64) {
        let Some(seat) = self.existing_seat(place.channel) else {
            return;
        };
        let mut seat = seat.lock().await;
        if seat.active.is_some() {
            return;
        }
        let Some((options, starter, length, position)) = seat
            .series
            .as_ref()
            .filter(|current| current.number == series)
            .map(|current| {
                (
                    current.options,
                    current.starter,
                    current.length,
                    current.played + 1,
                )
            })
        else {
            return;
        };
        let context = Some(SeriesRound::at(position));
        let outcome = self
            .open_round(
                place,
                &mut seat,
                &options,
                starter,
                context,
                |draw, number| async move {
                    let posting = self.announcer.post_series_round(
                        place.channel,
                        &draw,
                        number,
                        position,
                        length,
                    );
                    announced(
                        posting,
                        "a series round could not be posted",
                        "posting a series round timed out",
                        &round_scope(place, number),
                    )
                    .await
                    .ok_or(StartOutcome::PostFailed(()))
                },
            )
            .await;
        match outcome {
            StartOutcome::Started { .. } => {
                seat.series = seat.series.take().map(|current| Series {
                    played: position,
                    ..current
                });
            }
            StartOutcome::NoShips(error) => {
                post_failed(
                    "a series round could not be drawn",
                    &error,
                    &round_scope(place, series),
                );
                close_series(place, &mut seat, SERIES_UNPOSTED);
            }
            StartOutcome::Busy | StartOutcome::PostFailed(()) | StartOutcome::PostTimedOut => {
                close_series(place, &mut seat, SERIES_UNPOSTED);
            }
        }
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
        tokio::spawn(async move { table.run_timer(place, number).await });
    }

    fn spawn_count_down(
        self: &Arc<Self>,
        place: Place,
        series: u64,
        message: Option<Snowflake>,
        content: impl Fn(u32) -> String + Send + 'static,
    ) {
        let table = Arc::clone(self);
        tokio::spawn(async move { table.count_down(place, series, message, content).await });
    }

    async fn count_down(
        self: &Arc<Self>,
        place: Place,
        series: u64,
        message: Option<Snowflake>,
        content: impl Fn(u32) -> String,
    ) {
        let begun = tokio::time::Instant::now();
        for remaining in COUNTDOWN_EDITS {
            tokio::time::sleep_until(begun + whole_seconds(COUNTDOWN_SECONDS - remaining)).await;
            if let Some(message) = message {
                self.spawn_edit(place, series, message, content(remaining));
            }
        }
        tokio::time::sleep_until(begun + whole_seconds(COUNTDOWN_SECONDS)).await;
        self.next_series_round(place, series).await;
    }

    fn spawn_edit(
        self: &Arc<Self>,
        place: Place,
        series: u64,
        message: Snowflake,
        content: String,
    ) {
        let table = Arc::clone(self);
        tokio::spawn(async move { table.edit(place, series, message, &content).await });
    }

    async fn edit(&self, place: Place, series: u64, message: Snowflake, content: &str) {
        let editing = self
            .announcer
            .edit_series_message(place.channel, message, content);
        announced(
            editing,
            "a series message could not be edited",
            "editing a series message timed out",
            &round_scope(place, series).message(message),
        )
        .await;
    }

    fn spawn_expiry(self: &Arc<Self>, place: Place, series: u64) {
        let table = Arc::clone(self);
        tokio::spawn(async move {
            tokio::time::sleep(CHECK_WINDOW).await;
            table.expire(place, series).await;
        });
    }

    async fn expire(&self, place: Place, series: u64) {
        let Some(seat) = self.existing_seat(place.channel) else {
            return;
        };
        let mut seat = seat.lock().await;
        let Some((message, content)) = seat
            .series
            .as_ref()
            .filter(|current| current.number == series)
            .and_then(|current| {
                current.check.as_ref().map(|check| {
                    (
                        check.message,
                        text::series_message(&[
                            &check.result,
                            text::SERIES_EXPIRED,
                            &current.overview(),
                        ]),
                    )
                })
            })
        else {
            return;
        };
        if let Some(message) = message {
            self.edit(place, series, message, &content).await;
        }
        close_series(place, &mut seat, SERIES_WENT_QUIET);
    }

    async fn run_timer(self: &Arc<Self>, place: Place, number: u64) {
        tokio::time::sleep(self.timing.before_hint).await;
        if !self.post_hint(place, number).await {
            return;
        }
        tokio::time::sleep(self.timing.after_hint).await;
        self.time_out(place, number).await;
    }

    async fn post_hint(&self, place: Place, number: u64) -> bool {
        let Some(seat) = self.existing_seat(place.channel) else {
            return false;
        };
        let seat = seat.lock().await;
        let Some(active) = seat
            .active
            .as_ref()
            .filter(|active| active.number == number && active.leader.is_none())
        else {
            return false;
        };
        let posting = self
            .announcer
            .post_hint(place.channel, active.round.draw().hint());
        announced(
            posting,
            "a hint could not be posted",
            "posting a hint timed out",
            &round_scope(place, number),
        )
        .await;
        true
    }

    async fn time_out(self: &Arc<Self>, place: Place, number: u64) {
        let Some(seat) = self.existing_seat(place.channel) else {
            return;
        };
        let mut seat = seat.lock().await;
        if seat
            .active
            .as_ref()
            .is_none_or(|active| active.number != number || active.leader.is_some())
        {
            return;
        }
        let Some(active) = seat.active.take() else {
            return;
        };
        let ending = Ending::TimedOut {
            reveal: active.round.draw().reveal().clone(),
        };
        self.hand_off(place, &mut seat, &active, &ending).await;
    }

    async fn hand_off(
        self: &Arc<Self>,
        place: Place,
        seat: &mut Seat,
        active: &Active,
        ending: &Ending,
    ) {
        let scope = round_scope(place, active.number);
        failure::record("guess.round.ended", "a silhouette round ended", &scope);
        match active.series {
            None => {
                let posting =
                    self.announcer
                        .post_ending(place.channel, active.round.posted(), ending);
                announced(
                    posting,
                    "a round ending could not be posted",
                    "posting a round ending timed out",
                    &scope,
                )
                .await;
            }
            Some(context) => {
                self.series_ending(place, seat, active, context, ending)
                    .await;
            }
        }
    }

    async fn series_ending(
        self: &Arc<Self>,
        place: Place,
        seat: &mut Seat,
        active: &Active,
        context: SeriesRound,
        ending: &Ending,
    ) {
        let Some(series) = seat.series.take() else {
            return;
        };
        let series = series.after(context.heard, ending);
        let over = context.ends_series || series.played >= series.length.get();
        let check = !over && series.quiet.check_due();
        let result = match ending {
            Ending::Cancelled { by, reveal } if context.ends_series => {
                text::series_ended(*by, reveal)
            }
            Ending::Cancelled { by, reveal } => text::skipped(*by, reveal),
            Ending::Solved { .. } | Ending::TimedOut { .. } => text::ending_result(ending),
        };
        let standings = if over {
            series.overview()
        } else {
            series.standings()
        };
        let next = context.position + 1;
        let tail = if over {
            String::new()
        } else if check {
            text::SERIES_CHECK.to_owned()
        } else {
            text::series_countdown(next, series.length, COUNTDOWN_SECONDS)
        };
        let content = text::series_message(&[&result, &standings, &tail]);
        let posting = self.announcer.post_series_ending(
            place.channel,
            active.round.posted(),
            &content,
            ending.winning_message(),
            check.then_some(series.number),
        );
        let posted = announced(
            posting,
            "a series round ending could not be posted",
            "posting a series round ending timed out",
            &round_scope(place, active.number),
        )
        .await;
        let number = series.number;
        let length = series.length;
        if over {
            seat.series = Some(series);
            let reason = if context.ends_series {
                SERIES_ENDED_BY_PLAYER
            } else {
                SERIES_COMPLETED
            };
            close_series(place, seat, reason);
        } else if check {
            seat.series = Some(Series {
                check: Some(Check {
                    message: posted,
                    result,
                }),
                ..series
            });
            self.spawn_expiry(place, number);
        } else {
            seat.series = Some(series);
            self.spawn_count_down(
                place,
                number,
                posted,
                countdown_content(result, standings, next, length),
            );
        }
    }
}

fn countdown_content(
    result: String,
    standings: String,
    next: u32,
    length: SeriesLength,
) -> impl Fn(u32) -> String + Send + 'static {
    move |remaining| {
        text::series_message(&[
            &result,
            &standings,
            &text::series_countdown(next, length, remaining),
        ])
    }
}

fn close_series(place: Place, seat: &mut Seat, reason: &str) {
    if let Some(series) = seat.series.take() {
        failure::record(
            "guess.series.ended",
            reason,
            &round_scope(place, series.number),
        );
    }
}

async fn command_post<E>(
    posting: impl Future<Output = Result<Snowflake, E>>,
) -> Result<Snowflake, StartOutcome<E>> {
    match tokio::time::timeout(ANNOUNCE_TIMEOUT, posting).await {
        Ok(Ok(posted)) => Ok(posted),
        Ok(Err(error)) => Err(StartOutcome::PostFailed(error)),
        Err(_) => Err(StartOutcome::PostTimedOut),
    }
}

async fn announced<T>(
    posting: impl Future<Output = Result<T, AnnounceError>>,
    failed: &str,
    timed_out: &str,
    scope: &Scope,
) -> Option<T> {
    match tokio::time::timeout(ANNOUNCE_TIMEOUT, posting).await {
        Ok(Ok(value)) => Some(value),
        Ok(Err(error)) => {
            post_failed(failed, &error, scope);
            None
        }
        Err(elapsed) => {
            post_failed(timed_out, &elapsed, scope);
            None
        }
    }
}

fn whole_seconds(value: u32) -> Duration {
    Duration::from_secs(u64::from(value))
}

fn round_scope(place: Place, number: u64) -> Scope {
    Scope {
        round: u32::try_from(number).ok(),
        ..Scope::default().guild(place.guild).channel(place.channel)
    }
}

fn post_failed(summary: &str, error: &(dyn std::error::Error + 'static), scope: &Scope) {
    failure::report(
        "guess.post.failed",
        summary,
        &Failure::from_error(error),
        scope,
    );
}

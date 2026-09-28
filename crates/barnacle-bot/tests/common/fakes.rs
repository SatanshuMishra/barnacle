use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;

use barnacle_bot::attendance::Board;
use barnacle_bot::attendance::BoardError;
use barnacle_bot::attendance::Delivery;
use barnacle_bot::attendance::PostTag;
use barnacle_bot::attendance::Removal;
use barnacle_bot::attendance::Sent;
use barnacle_bot::attendance::SignupView;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::Ping;
use barnacle_bot::ids::Place;
use barnacle_bot::solves::SolveRecord;
use barnacle_bot::solves::SolveStore;
use barnacle_bot::solves::SolvesError;
use barnacle_bot::table::AnnounceError;
use barnacle_bot::table::Announcer;
use barnacle_bot::table::Ending;
use barnacle_bot::table::Table;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipIndex;
use barnacle_guess::Draw;
use barnacle_guess::Hint;
use barnacle_guess::SeriesLength;
use barnacle_guess::ShipBook;
use barnacle_guess::Snowflake;
use barnacle_guess::Timing;
use rand::SeedableRng;
use rand::rngs::StdRng;
use tokio::sync::Semaphore;
use tokio::time::Instant;

use super::at;
use super::catalog;
use super::curation;

pub const PLACE: Place = Place {
    guild: GuildId::new(1),
    channel: ChannelId::new(10),
};
pub const OTHER_PLACE: Place = Place {
    guild: GuildId::new(1),
    channel: ChannelId::new(11),
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Posted {
    Hint {
        channel: ChannelId,
        hint: Hint,
    },
    Ending {
        channel: ChannelId,
        round_post: Snowflake,
        ending: Ending,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeriesPosted {
    Round {
        channel: ChannelId,
        number: u64,
        round: u32,
        length: SeriesLength,
        ship: ShipIndex,
    },
    Ending {
        channel: ChannelId,
        round_post: Snowflake,
        content: String,
        reply_to: Option<Snowflake>,
        still_playing: Option<u64>,
    },
    Edited {
        channel: ChannelId,
        message: Snowflake,
        content: String,
    },
}

type Log<T> = Arc<Mutex<Vec<(Duration, T)>>>;

#[derive(Clone)]
pub struct FakeDiscord {
    start: Instant,
    fail: bool,
    hang: bool,
    posted: Log<Posted>,
    series_posted: Log<SeriesPosted>,
}

impl FakeDiscord {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            fail: false,
            hang: false,
            posted: Arc::new(Mutex::new(Vec::new())),
            series_posted: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn failing() -> Self {
        Self {
            fail: true,
            ..Self::new()
        }
    }

    pub fn hanging() -> Self {
        Self {
            hang: true,
            ..Self::new()
        }
    }

    pub fn posted(&self) -> Vec<(Duration, Posted)> {
        self.posted.lock().unwrap().clone()
    }

    pub fn endings(&self) -> Vec<Ending> {
        self.posted()
            .into_iter()
            .filter_map(|(_, posted)| match posted {
                Posted::Ending { ending, .. } => Some(ending),
                Posted::Hint { .. } => None,
            })
            .collect()
    }

    pub fn series_posted(&self) -> Vec<(Duration, SeriesPosted)> {
        self.series_posted.lock().unwrap().clone()
    }

    fn now(&self) -> Snowflake {
        at(u64::try_from(self.start.elapsed().as_millis()).unwrap())
    }

    async fn answer<T>(
        &self,
        log: &Mutex<Vec<(Duration, T)>>,
        call: T,
    ) -> Result<(), AnnounceError> {
        let result = self.record(log, call);
        if self.hang {
            std::future::pending::<()>().await;
        }
        result
    }

    fn record<T>(&self, log: &Mutex<Vec<(Duration, T)>>, call: T) -> Result<(), AnnounceError> {
        log.lock().unwrap().push((self.start.elapsed(), call));
        if self.fail {
            Err(AnnounceError("the fake Discord refuses every post".into()))
        } else {
            Ok(())
        }
    }
}

impl Announcer for FakeDiscord {
    async fn post_hint(&self, channel: ChannelId, hint: &Hint) -> Result<(), AnnounceError> {
        self.answer(
            &self.posted,
            Posted::Hint {
                channel,
                hint: hint.clone(),
            },
        )
        .await
    }

    async fn post_ending(
        &self,
        channel: ChannelId,
        round_post: Snowflake,
        ending: &Ending,
    ) -> Result<(), AnnounceError> {
        self.answer(
            &self.posted,
            Posted::Ending {
                channel,
                round_post,
                ending: ending.clone(),
            },
        )
        .await
    }

    async fn post_series_round(
        &self,
        channel: ChannelId,
        draw: &Draw,
        number: u64,
        round: u32,
        length: SeriesLength,
    ) -> Result<Snowflake, AnnounceError> {
        let posted = self.now();
        self.answer(
            &self.series_posted,
            SeriesPosted::Round {
                channel,
                number,
                round,
                length,
                ship: draw.ship().clone(),
            },
        )
        .await
        .map(|()| posted)
    }

    async fn post_series_ending(
        &self,
        channel: ChannelId,
        round_post: Snowflake,
        content: &str,
        reply_to: Option<Snowflake>,
        still_playing: Option<u64>,
    ) -> Result<Snowflake, AnnounceError> {
        let posted = self.now();
        self.answer(
            &self.series_posted,
            SeriesPosted::Ending {
                channel,
                round_post,
                content: content.to_owned(),
                reply_to,
                still_playing,
            },
        )
        .await
        .map(|()| posted)
    }

    async fn edit_series_message(
        &self,
        channel: ChannelId,
        message: Snowflake,
        content: &str,
    ) -> Result<(), AnnounceError> {
        self.answer(
            &self.series_posted,
            SeriesPosted::Edited {
                channel,
                message,
                content: content.to_owned(),
            },
        )
        .await
    }
}

#[derive(Clone, Default)]
pub struct FakeStore {
    fail: bool,
    saved: Arc<Mutex<Vec<SolveRecord>>>,
}

impl FakeStore {
    pub fn failing() -> Self {
        Self {
            fail: true,
            ..Self::default()
        }
    }

    pub fn saved(&self) -> Vec<SolveRecord> {
        self.saved.lock().unwrap().clone()
    }
}

impl SolveStore for FakeStore {
    async fn record(&self, record: &SolveRecord) -> Result<bool, SolvesError> {
        if self.fail {
            return Err(SolvesError::MissingTable);
        }
        let mut saved = self.saved.lock().unwrap();
        let best = saved
            .iter()
            .filter(|earlier| earlier.guild == record.guild && earlier.user == record.user)
            .map(|earlier| earlier.elapsed)
            .min();
        saved.push(record.clone());
        Ok(best.is_none_or(|best| record.elapsed < best))
    }
}

pub fn table_with(
    ships: Vec<Ship>,
    discord: FakeDiscord,
    store: FakeStore,
) -> Arc<Table<FakeDiscord, FakeStore>> {
    let book = ShipBook::new(&catalog(ships), &curation(""));
    Table::new(
        book,
        store,
        discord,
        Timing::STANDARD,
        StdRng::seed_from_u64(7),
    )
}

const FIRST_MESSAGE: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardCall {
    Sent {
        channel: ChannelId,
        view: SignupView,
        delivery: Delivery,
    },
    Looked {
        channel: ChannelId,
        tag: PostTag,
    },
    Edited {
        channel: ChannelId,
        message: Snowflake,
        view: SignupView,
    },
    Deleted {
        channel: ChannelId,
        message: Snowflake,
    },
}

struct BoardState {
    calls: Mutex<Vec<BoardCall>>,
    planted: Mutex<Vec<(PostTag, Snowflake)>>,
    next_message: AtomicU64,
    fail_sends: AtomicBool,
    fail_edits: AtomicBool,
    fail_deletes: AtomicBool,
    fail_deletes_in: Mutex<Vec<ChannelId>>,
    report_gone: AtomicBool,
    unheard_pings: AtomicBool,
    unheard_only: Mutex<Vec<Ping>>,
    gate: Semaphore,
    send_gate: Mutex<Option<Arc<Semaphore>>>,
}

#[derive(Clone)]
pub struct FakeBoard {
    state: Arc<BoardState>,
}

impl FakeBoard {
    pub fn new() -> Self {
        let board = Self::gated();
        board.open_edits();
        board
    }

    pub fn gated() -> Self {
        Self {
            state: Arc::new(BoardState {
                calls: Mutex::new(Vec::new()),
                planted: Mutex::new(Vec::new()),
                next_message: AtomicU64::new(FIRST_MESSAGE),
                fail_sends: AtomicBool::new(false),
                fail_edits: AtomicBool::new(false),
                fail_deletes: AtomicBool::new(false),
                fail_deletes_in: Mutex::new(Vec::new()),
                report_gone: AtomicBool::new(false),
                unheard_pings: AtomicBool::new(false),
                unheard_only: Mutex::new(Vec::new()),
                gate: Semaphore::new(0),
                send_gate: Mutex::new(None),
            }),
        }
    }

    pub fn open_edits(&self) {
        self.state.gate.close();
    }

    pub fn gate_sends(&self) {
        *self.state.send_gate.lock().unwrap() = Some(Arc::new(Semaphore::new(0)));
    }

    pub fn open_sends(&self) {
        if let Some(gate) = self.state.send_gate.lock().unwrap().take() {
            gate.close();
        }
    }

    pub fn fail_sends(&self, fail: bool) {
        self.state.fail_sends.store(fail, SeqCst);
    }

    pub fn fail_edits(&self, fail: bool) {
        self.state.fail_edits.store(fail, SeqCst);
    }

    pub fn fail_deletes(&self, fail: bool) {
        self.state.fail_deletes.store(fail, SeqCst);
    }

    pub fn fail_deletes_in(&self, channel: ChannelId) {
        self.state.fail_deletes_in.lock().unwrap().push(channel);
    }

    pub fn report_gone(&self, gone: bool) {
        self.state.report_gone.store(gone, SeqCst);
    }

    pub fn unheard_pings(&self, unheard: bool) {
        self.state.unheard_pings.store(unheard, SeqCst);
    }

    pub fn unheard_only(&self, pings: Vec<Ping>) {
        *self.state.unheard_only.lock().unwrap() = pings;
    }

    pub fn plant(&self, tag: PostTag, message: Snowflake) {
        self.state.planted.lock().unwrap().push((tag, message));
    }

    pub fn calls(&self) -> Vec<BoardCall> {
        self.state.calls.lock().unwrap().clone()
    }

    pub fn sends(&self) -> Vec<SignupView> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                BoardCall::Sent { view, .. } => Some(view),
                _ => None,
            })
            .collect()
    }

    pub fn sends_to(&self, wanted: ChannelId) -> Vec<SignupView> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                BoardCall::Sent { channel, view, .. } if channel == wanted => Some(view),
                _ => None,
            })
            .collect()
    }

    pub fn edits(&self) -> Vec<SignupView> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                BoardCall::Edited { view, .. } => Some(view),
                _ => None,
            })
            .collect()
    }

    pub fn deletes(&self) -> Vec<Snowflake> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                BoardCall::Deleted { message, .. } => Some(message),
                _ => None,
            })
            .collect()
    }

    pub fn deletes_in(&self, wanted: ChannelId) -> Vec<Snowflake> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                BoardCall::Deleted { channel, message } if channel == wanted => Some(message),
                _ => None,
            })
            .collect()
    }

    pub fn pinging_sends(&self) -> Vec<SignupView> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                BoardCall::Sent {
                    view,
                    delivery: Delivery::New,
                    ..
                } if !view.pings.is_empty() => Some(view),
                _ => None,
            })
            .collect()
    }

    fn record(&self, call: BoardCall) {
        self.state.calls.lock().unwrap().push(call);
    }
}

impl Default for FakeBoard {
    fn default() -> Self {
        Self::new()
    }
}

impl Board for FakeBoard {
    async fn send_post(
        &self,
        channel: ChannelId,
        view: &SignupView,
        delivery: Delivery,
    ) -> Result<Sent, BoardError> {
        self.record(BoardCall::Sent {
            channel,
            view: view.clone(),
            delivery,
        });
        let gate = self.state.send_gate.lock().unwrap().clone();
        if let Some(gate) = gate {
            let _held = gate.acquire().await;
        }
        if self.state.fail_sends.load(SeqCst) {
            return Err(BoardError("the fake board refuses every send".into()));
        }
        let only = self.state.unheard_only.lock().unwrap().clone();
        let unheard = view
            .pings
            .iter()
            .copied()
            .filter(|ping| self.state.unheard_pings.load(SeqCst) || only.contains(ping))
            .collect();
        Ok(Sent {
            message: Snowflake::new(self.state.next_message.fetch_add(1, SeqCst)),
            unheard,
        })
    }

    async fn find_post(
        &self,
        channel: ChannelId,
        tag: PostTag,
    ) -> Result<Option<Snowflake>, BoardError> {
        self.record(BoardCall::Looked { channel, tag });
        let planted = self.state.planted.lock().unwrap();
        Ok(planted
            .iter()
            .find(|(placed, _)| *placed == tag)
            .map(|(_, message)| *message))
    }

    async fn edit_post(
        &self,
        channel: ChannelId,
        message: Snowflake,
        view: &SignupView,
    ) -> Result<(), BoardError> {
        let _held = self.state.gate.acquire().await;
        self.record(BoardCall::Edited {
            channel,
            message,
            view: view.clone(),
        });
        if self.state.fail_edits.load(SeqCst) {
            return Err(BoardError("the fake board refuses every edit".into()));
        }
        Ok(())
    }

    async fn delete_post(
        &self,
        channel: ChannelId,
        message: Snowflake,
    ) -> Result<Removal, BoardError> {
        self.record(BoardCall::Deleted { channel, message });
        if self.state.fail_deletes.load(SeqCst)
            || self
                .state
                .fail_deletes_in
                .lock()
                .unwrap()
                .contains(&channel)
        {
            return Err(BoardError("the fake board refuses every delete".into()));
        }
        if self.state.report_gone.load(SeqCst) {
            Ok(Removal::Gone)
        } else {
            Ok(Removal::Deleted)
        }
    }
}

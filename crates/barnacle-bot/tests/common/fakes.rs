use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::Place;
use barnacle_bot::solves::SolveRecord;
use barnacle_bot::solves::SolveStore;
use barnacle_bot::solves::SolvesError;
use barnacle_bot::table::AnnounceError;
use barnacle_bot::table::Announcer;
use barnacle_bot::table::Ending;
use barnacle_bot::table::Table;
use barnacle_catalog::Ship;
use barnacle_guess::Hint;
use barnacle_guess::ShipBook;
use barnacle_guess::Snowflake;
use barnacle_guess::Timing;
use rand::SeedableRng;
use rand::rngs::StdRng;
use tokio::time::Instant;

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

#[derive(Clone)]
pub struct FakeDiscord {
    start: Instant,
    fail: bool,
    posted: Arc<Mutex<Vec<(Duration, Posted)>>>,
}

impl FakeDiscord {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            fail: false,
            posted: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn failing() -> Self {
        Self {
            fail: true,
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

    fn record(&self, posted: Posted) -> Result<(), AnnounceError> {
        self.posted
            .lock()
            .unwrap()
            .push((self.start.elapsed(), posted));
        if self.fail {
            Err(AnnounceError("the fake Discord refuses every post".into()))
        } else {
            Ok(())
        }
    }
}

impl Announcer for FakeDiscord {
    async fn post_hint(&self, channel: ChannelId, hint: &Hint) -> Result<(), AnnounceError> {
        self.record(Posted::Hint {
            channel,
            hint: hint.clone(),
        })
    }

    async fn post_ending(
        &self,
        channel: ChannelId,
        round_post: Snowflake,
        ending: &Ending,
    ) -> Result<(), AnnounceError> {
        self.record(Posted::Ending {
            channel,
            round_post,
            ending: ending.clone(),
        })
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

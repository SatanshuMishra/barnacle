use std::time::Duration;

use crate::ids::UserId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SeriesLength(u32);

impl SeriesLength {
    pub const MIN: u32 = 2;
    pub const MAX: u32 = 20;
    pub const DEFAULT: u32 = 10;

    pub fn new(value: u32) -> Option<Self> {
        (Self::MIN..=Self::MAX)
            .contains(&value)
            .then_some(Self(value))
    }

    pub fn get(&self) -> u32 {
        self.0
    }
}

impl Default for SeriesLength {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeriesStanding {
    pub user: UserId,
    pub wins: u32,
    pub best: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    user: UserId,
    wins: u32,
    best: Duration,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tally(Vec<Entry>);

impl Tally {
    pub fn won(&self, winner: UserId, elapsed: Duration) -> Self {
        let mut entries = self.0.clone();
        match entries.iter_mut().find(|entry| entry.user == winner) {
            Some(entry) => {
                entry.wins += 1;
                entry.best = entry.best.min(elapsed);
            }
            None => entries.push(Entry {
                user: winner,
                wins: 1,
                best: elapsed,
            }),
        }
        Self(entries)
    }

    pub fn by_wins(&self) -> Vec<SeriesStanding> {
        self.ordered(|a, b| {
            b.1.wins
                .cmp(&a.1.wins)
                .then(a.1.best.cmp(&b.1.best))
                .then(a.0.cmp(&b.0))
        })
    }

    pub fn by_time(&self) -> Vec<SeriesStanding> {
        self.ordered(|a, b| a.1.best.cmp(&b.1.best).then(a.0.cmp(&b.0)))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn ordered(
        &self,
        compare: impl FnMut(&(usize, &Entry), &(usize, &Entry)) -> std::cmp::Ordering,
    ) -> Vec<SeriesStanding> {
        let mut standings: Vec<_> = self.0.iter().enumerate().collect();
        standings.sort_by(compare);
        standings
            .into_iter()
            .map(|(_, entry)| SeriesStanding {
                user: entry.user,
                wins: entry.wins,
                best: entry.best,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Quiet(u32);

impl Quiet {
    pub const LIMIT: u32 = 3;

    pub fn after_round(&self, active: bool) -> Self {
        if active { Self(0) } else { Self(self.0 + 1) }
    }

    pub fn check_due(&self) -> bool {
        self.0 >= Self::LIMIT
    }
}

use std::time::Duration;

const TIMESTAMP_SHIFT: u32 = 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserId(u64);

impl UserId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Snowflake(u64);

impl Snowflake {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn millis_since_discord_epoch(self) -> u64 {
        self.0 >> TIMESTAMP_SHIFT
    }

    pub fn elapsed_until(self, later: Snowflake) -> Duration {
        Duration::from_millis(
            later
                .millis_since_discord_epoch()
                .saturating_sub(self.millis_since_discord_epoch()),
        )
    }
}

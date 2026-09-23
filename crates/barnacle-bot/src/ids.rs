use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuildId(u64);

impl GuildId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelId(u64);

impl ChannelId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoleId(u64);

impl RoleId {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for RoleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Place {
    pub guild: GuildId,
    pub channel: ChannelId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ping {
    Everyone,
    Role(RoleId),
}

impl Ping {
    pub const fn of(guild: GuildId, role: RoleId) -> Ping {
        if role.get() == guild.get() {
            Ping::Everyone
        } else {
            Ping::Role(role)
        }
    }
}

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::RoleId;
use crate::voice_store::Hub;
use crate::voice_store::Room;
use crate::voice_store::VoiceStore;

pub const EMPTY_GRACE_MS: u64 = 300_000;
pub const NEW_CHANNEL_GRACE_MS: u64 = 60_000;
pub const ROOM_NAME_LIMIT: usize = 90;
pub const HUB_NAME_LIMIT: usize = 100;
pub const ROOM_ACCESS: u64 = serenity::Permissions::VIEW_CHANNEL.bits()
    | serenity::Permissions::CONNECT.bits()
    | serenity::Permissions::MOVE_MEMBERS.bits()
    | serenity::Permissions::MANAGE_CHANNELS.bits();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameProblem {
    Empty,
    TooLong { limit: usize },
}

pub fn clean_name(raw: &str, limit: usize) -> Result<String, NameProblem> {
    let name = raw.trim();
    if name.is_empty() {
        Err(NameProblem::Empty)
    } else if name.chars().count() > limit {
        Err(NameProblem::TooLong { limit })
    } else {
        Ok(name.to_owned())
    }
}

pub fn room_channel_name(room_name: &str, number: u32) -> String {
    format!("{room_name}-{number}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverwriteTarget {
    Role(RoleId),
    Member(UserId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overwrite {
    pub target: OverwriteTarget,
    pub allow: u64,
    pub deny: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubLayout {
    pub category: Option<ChannelId>,
    pub position: u16,
    pub bitrate: Option<u32>,
    pub user_limit: Option<u32>,
    pub rtc_region: Option<String>,
    pub video_quality: Option<u8>,
    pub overwrites: Vec<Overwrite>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomSpec {
    pub name: String,
    pub layout: HubLayout,
}

pub fn with_room_access(overwrites: &[Overwrite], bot: UserId) -> Vec<Overwrite> {
    let own = OverwriteTarget::Member(bot);
    if overwrites.iter().any(|overwrite| overwrite.target == own) {
        overwrites
            .iter()
            .map(|overwrite| {
                if overwrite.target == own {
                    Overwrite {
                        target: own,
                        allow: overwrite.allow | ROOM_ACCESS,
                        deny: overwrite.deny & !ROOM_ACCESS,
                    }
                } else {
                    *overwrite
                }
            })
            .collect()
    } else {
        overwrites
            .iter()
            .copied()
            .chain(std::iter::once(Overwrite {
                target: own,
                allow: ROOM_ACCESS,
                deny: 0,
            }))
            .collect()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("the Discord call failed")]
pub struct RoomsError(#[source] pub Box<dyn std::error::Error + Send + Sync>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveOutcome {
    Moved,
    NotInVoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomRemoval {
    Deleted,
    Gone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Occupancy {
    Unknown,
    Missing,
    Empty,
    Occupied(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinOutcome {
    Opened { room: ChannelId, number: u32 },
    Abandoned,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SweepReport {
    pub closed: usize,
    pub forgotten: usize,
    pub failures: usize,
}

pub trait Rooms: Send + Sync + 'static {
    fn create_room(
        &self,
        guild: GuildId,
        spec: &RoomSpec,
    ) -> impl Future<Output = Result<ChannelId, RoomsError>> + Send;

    fn move_member(
        &self,
        guild: GuildId,
        user: UserId,
        room: ChannelId,
    ) -> impl Future<Output = Result<MoveOutcome, RoomsError>> + Send;

    fn delete_room(
        &self,
        room: ChannelId,
    ) -> impl Future<Output = Result<RoomRemoval, RoomsError>> + Send;
}

pub struct VoiceRooms<R: Rooms> {
    rooms: R,
    store: VoiceStore,
    guild_locks: std::sync::Mutex<HashMap<GuildId, Arc<tokio::sync::Mutex<()>>>>,
}

impl<R: Rooms> VoiceRooms<R> {
    pub fn new(rooms: R, store: VoiceStore) -> Arc<Self> {
        Arc::new(Self {
            rooms,
            store,
            guild_locks: std::sync::Mutex::new(HashMap::new()),
        })
    }

    pub fn store(&self) -> &VoiceStore {
        &self.store
    }

    pub async fn joined(
        &self,
        hub: &Hub,
        user: UserId,
        layout: &HubLayout,
        now_ms: u64,
    ) -> JoinOutcome {
        let lock = self.guild_lock(hub.guild);
        let opened = {
            let _numbering = lock.lock().await;
            self.open(hub, user, layout, now_ms).await
        };
        let Some((room, number)) = opened else {
            return JoinOutcome::Failed;
        };
        match self.rooms.move_member(hub.guild, user, room).await {
            Ok(MoveOutcome::Moved) => JoinOutcome::Opened { room, number },
            Ok(MoveOutcome::NotInVoice) => {
                self.abandon(room).await;
                JoinOutcome::Abandoned
            }
            Err(error) => {
                tracing::warn!(
                    %error,
                    room = room.get(),
                    guild = hub.guild.get(),
                    "a member could not be moved into their room"
                );
                self.abandon(room).await;
                JoinOutcome::Abandoned
            }
        }
    }

    pub async fn left(&self, channel: ChannelId, occupants: usize, now_ms: u64) {
        if occupants != 0 {
            return;
        }
        if let Err(error) = self.store.mark_empty(channel, now_ms).await {
            tracing::warn!(%error, channel = channel.get(), "an emptied room could not be marked");
        }
    }

    pub async fn entered(&self, channel: ChannelId) {
        if let Err(error) = self.store.mark_occupied(channel).await {
            tracing::warn!(%error, channel = channel.get(), "an entered room could not be marked");
        }
    }

    pub async fn channel_deleted(&self, channel: ChannelId) {
        let forgotten = match self.store.remove_room(channel).await {
            Ok(true) => Ok(true),
            Ok(false) => self.store.remove_hub(channel).await,
            Err(error) => Err(error),
        };
        if let Err(error) = forgotten {
            tracing::warn!(%error, channel = channel.get(), "a deleted channel could not be forgotten");
        }
    }

    pub async fn sweep(
        &self,
        occupancy: &(dyn Fn(GuildId, ChannelId) -> Occupancy + Sync),
        now_ms: u64,
    ) -> SweepReport {
        let mut report = SweepReport::default();
        match self.store.hubs().await {
            Ok(hubs) => {
                for hub in hubs
                    .iter()
                    .filter(|hub| settled(hub.created_at_ms, now_ms))
                    .filter(|hub| occupancy(hub.guild, hub.channel) == Occupancy::Missing)
                {
                    if let Err(error) = self.store.remove_hub(hub.channel).await {
                        tracing::warn!(%error, hub = hub.channel.get(), "a vanished Join to Create channel could not be forgotten");
                        report.failures += 1;
                    }
                }
            }
            Err(error) => {
                tracing::warn!(%error, "the Join to Create channels could not be read");
                report.failures += 1;
            }
        }
        let rooms = match self.store.rooms().await {
            Ok(rooms) => rooms,
            Err(error) => {
                tracing::warn!(%error, "the open rooms could not be read");
                report.failures += 1;
                return report;
            }
        };
        for room in &rooms {
            match occupancy(room.guild, room.channel) {
                Occupancy::Unknown => {}
                Occupancy::Missing if !settled(room.created_at_ms, now_ms) => {}
                Occupancy::Missing => match self.store.remove_room(room.channel).await {
                    Ok(_) => report.forgotten += 1,
                    Err(error) => {
                        tracing::warn!(%error, room = room.channel.get(), "a vanished room could not be forgotten");
                        report.failures += 1;
                    }
                },
                Occupancy::Occupied(_) => {
                    if room.empty_since_ms.is_some()
                        && let Err(error) = self.store.mark_occupied(room.channel).await
                    {
                        tracing::warn!(%error, room = room.channel.get(), "an occupied room could not be marked");
                        report.failures += 1;
                    }
                }
                Occupancy::Empty => match room.empty_since_ms {
                    None => {
                        if let Err(error) = self.store.mark_empty(room.channel, now_ms).await {
                            tracing::warn!(%error, room = room.channel.get(), "an empty room could not be marked");
                            report.failures += 1;
                        }
                    }
                    Some(since) if now_ms.saturating_sub(since) >= EMPTY_GRACE_MS => {
                        if self.close(room).await {
                            report.closed += 1;
                        } else {
                            report.failures += 1;
                        }
                    }
                    Some(_) => {}
                },
            }
        }
        report
    }

    async fn open(
        &self,
        hub: &Hub,
        user: UserId,
        layout: &HubLayout,
        now_ms: u64,
    ) -> Option<(ChannelId, u32)> {
        let number = match self.store.next_number(hub.guild, &hub.room_name).await {
            Ok(number) => number,
            Err(error) => {
                tracing::warn!(%error, hub = hub.channel.get(), "the next room number could not be read");
                return None;
            }
        };
        let spec = RoomSpec {
            name: room_channel_name(&hub.room_name, number),
            layout: layout.clone(),
        };
        let room = match self.rooms.create_room(hub.guild, &spec).await {
            Ok(room) => room,
            Err(error) => {
                tracing::warn!(%error, hub = hub.channel.get(), guild = hub.guild.get(), "Discord refused to create a room");
                return None;
            }
        };
        let record = Room {
            channel: room,
            guild: hub.guild,
            hub: hub.channel,
            room_name: hub.room_name.clone(),
            number,
            owner: user,
            created_at_ms: now_ms,
            empty_since_ms: None,
        };
        if let Err(error) = self.store.add_room(&record).await {
            tracing::warn!(%error, room = room.get(), "a new room could not be recorded");
            if let Err(cleanup) = self.rooms.delete_room(room).await {
                tracing::error!(%cleanup, room = room.get(), "an unrecorded room could not be deleted");
            }
            return None;
        }
        Some((room, number))
    }

    async fn abandon(&self, room: ChannelId) {
        match self.rooms.delete_room(room).await {
            Ok(RoomRemoval::Deleted | RoomRemoval::Gone) => {
                if let Err(error) = self.store.remove_room(room).await {
                    tracing::warn!(%error, room = room.get(), "an abandoned room could not be forgotten");
                }
            }
            Err(error) => {
                tracing::warn!(%error, room = room.get(), "an abandoned room could not be deleted, so the sweep will close it");
            }
        }
    }

    async fn close(&self, room: &Room) -> bool {
        match self.rooms.delete_room(room.channel).await {
            Ok(RoomRemoval::Deleted | RoomRemoval::Gone) => {
                match self.store.remove_room(room.channel).await {
                    Ok(_) => true,
                    Err(error) => {
                        tracing::warn!(%error, room = room.channel.get(), "a closed room could not be forgotten");
                        false
                    }
                }
            }
            Err(error) => {
                tracing::warn!(%error, room = room.channel.get(), "an empty room could not be deleted");
                false
            }
        }
    }

    fn guild_lock(&self, guild: GuildId) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self
            .guild_locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::clone(locks.entry(guild).or_default())
    }
}

fn settled(created_at_ms: u64, now_ms: u64) -> bool {
    now_ms.saturating_sub(created_at_ms) >= NEW_CHANNEL_GRACE_MS
}

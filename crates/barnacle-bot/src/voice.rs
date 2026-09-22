use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use crate::failure;
use crate::failure::Failure;
use crate::failure::Scope;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::RoleId;
use crate::voice_store::Hub;
use crate::voice_store::Room;
use crate::voice_store::VoiceStore;

pub const EMPTY_GRACE_MS: u64 = 300_000;
pub const NEW_CHANNEL_GRACE_MS: u64 = 60_000;
pub const NOTICE_INTERVAL_MS: u64 = 60_000;
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
    pub blockers: Vec<&'static str>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinOutcome {
    Opened { room: ChannelId, number: u32 },
    Refused(Failure),
    Abandoned,
    Failed(Failure),
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

    fn notify(
        &self,
        hub: ChannelId,
        user: UserId,
        content: &str,
    ) -> impl Future<Output = Result<(), RoomsError>> + Send;
}

pub struct VoiceRooms<R: Rooms> {
    rooms: R,
    store: VoiceStore,
    guild_locks: std::sync::Mutex<HashMap<GuildId, Arc<tokio::sync::Mutex<()>>>>,
    notices: std::sync::Mutex<HashMap<ChannelId, u64>>,
}

impl<R: Rooms> VoiceRooms<R> {
    pub fn new(rooms: R, store: VoiceStore) -> Arc<Self> {
        Arc::new(Self {
            rooms,
            store,
            guild_locks: std::sync::Mutex::new(HashMap::new()),
            notices: std::sync::Mutex::new(HashMap::new()),
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
        let scope = hub_scope(hub).user(user);
        if !layout.blockers.is_empty() {
            let failure = Failure::missing_permissions(layout.blockers.clone());
            failure::report(
                "voice.room.refused",
                "a Join to Create room was not opened because Barnacle cannot copy the hub's permissions",
                &failure,
                &scope,
            );
            self.tell(hub, user, &failure, now_ms, &scope).await;
            return JoinOutcome::Refused(failure);
        }
        let lock = self.guild_lock(hub.guild);
        let opened = {
            let _numbering = lock.lock().await;
            self.open(hub, user, layout, now_ms, &scope).await
        };
        let (room, number) = match opened {
            Ok(opened) => opened,
            Err(failure) => {
                self.tell(hub, user, &failure, now_ms, &scope).await;
                return JoinOutcome::Failed(failure);
            }
        };
        let scope = scope.room(room).room_number(number);
        match self.rooms.move_member(hub.guild, user, room).await {
            Ok(MoveOutcome::Moved) => {
                failure::record("voice.room.opened", "a Join to Create room opened", &scope);
                JoinOutcome::Opened { room, number }
            }
            Ok(MoveOutcome::NotInVoice) => {
                failure::record(
                    "voice.room.abandoned",
                    "a member left voice before Barnacle could move them into their new room, so it was closed",
                    &scope,
                );
                self.abandon(room, &scope).await;
                JoinOutcome::Abandoned
            }
            Err(error) => {
                failure::report(
                    "voice.room.abandoned",
                    "a member could not be moved into their new room, so it was closed",
                    &Failure::from_error(&error),
                    &scope,
                );
                self.abandon(room, &scope).await;
                JoinOutcome::Abandoned
            }
        }
    }

    pub async fn left(&self, channel: ChannelId, occupants: usize, now_ms: u64) {
        if occupants != 0 {
            return;
        }
        if let Err(error) = self.store.mark_empty(channel, now_ms).await {
            failure::report(
                "voice.state.failed",
                "an emptied room could not be marked",
                &Failure::from_error(&error),
                &Scope::default().room(channel),
            );
        }
    }

    pub async fn entered(&self, channel: ChannelId) {
        if let Err(error) = self.store.mark_occupied(channel).await {
            failure::report(
                "voice.state.failed",
                "an entered room could not be marked",
                &Failure::from_error(&error),
                &Scope::default().room(channel),
            );
        }
    }

    pub async fn channel_deleted(&self, channel: ChannelId) {
        match self.store.remove_room(channel).await {
            Ok(true) => failure::record(
                "voice.room.forgotten",
                "a deleted room was forgotten",
                &Scope::default().room(channel),
            ),
            Ok(false) => match self.store.remove_hub(channel).await {
                Ok(true) => failure::record(
                    "voice.hub.forgotten",
                    "a deleted Join to Create channel was forgotten",
                    &Scope::default().hub(channel),
                ),
                Ok(false) => {}
                Err(error) => failure::report(
                    "voice.hub.forgotten",
                    "a deleted Join to Create channel could not be forgotten",
                    &Failure::from_error(&error),
                    &Scope::default().hub(channel),
                ),
            },
            Err(error) => failure::report(
                "voice.room.forgotten",
                "a deleted room could not be forgotten",
                &Failure::from_error(&error),
                &Scope::default().room(channel),
            ),
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
                    match self.store.remove_hub(hub.channel).await {
                        Ok(_) => failure::record(
                            "voice.hub.forgotten",
                            "a vanished Join to Create channel was forgotten",
                            &hub_scope(hub),
                        ),
                        Err(error) => {
                            failure::report(
                                "voice.hub.forgotten",
                                "a vanished Join to Create channel could not be forgotten",
                                &Failure::from_error(&error),
                                &hub_scope(hub),
                            );
                            report.failures += 1;
                        }
                    }
                }
            }
            Err(error) => {
                failure::report(
                    "voice.state.failed",
                    "the Join to Create channels could not be read",
                    &Failure::from_error(&error),
                    &Scope::default(),
                );
                report.failures += 1;
            }
        }
        let rooms = match self.store.rooms().await {
            Ok(rooms) => rooms,
            Err(error) => {
                failure::report(
                    "voice.state.failed",
                    "the open rooms could not be read",
                    &Failure::from_error(&error),
                    &Scope::default(),
                );
                report.failures += 1;
                return report;
            }
        };
        for room in &rooms {
            let scope = room_scope(room);
            match occupancy(room.guild, room.channel) {
                Occupancy::Unknown => {}
                Occupancy::Missing if !settled(room.created_at_ms, now_ms) => {}
                Occupancy::Missing => match self.store.remove_room(room.channel).await {
                    Ok(_) => {
                        failure::record(
                            "voice.room.forgotten",
                            "a vanished room was forgotten",
                            &scope,
                        );
                        report.forgotten += 1;
                    }
                    Err(error) => {
                        failure::report(
                            "voice.room.forgotten",
                            "a vanished room could not be forgotten",
                            &Failure::from_error(&error),
                            &scope,
                        );
                        report.failures += 1;
                    }
                },
                Occupancy::Occupied(_) => {
                    if room.empty_since_ms.is_some()
                        && let Err(error) = self.store.mark_occupied(room.channel).await
                    {
                        failure::report(
                            "voice.state.failed",
                            "an occupied room could not be marked",
                            &Failure::from_error(&error),
                            &scope,
                        );
                        report.failures += 1;
                    }
                }
                Occupancy::Empty => match room.empty_since_ms {
                    None => {
                        if let Err(error) = self.store.mark_empty(room.channel, now_ms).await {
                            failure::report(
                                "voice.state.failed",
                                "an empty room could not be marked",
                                &Failure::from_error(&error),
                                &scope,
                            );
                            report.failures += 1;
                        }
                    }
                    Some(since) if now_ms.saturating_sub(since) >= EMPTY_GRACE_MS => {
                        if self.close(room, &scope).await {
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
        scope: &Scope,
    ) -> Result<(ChannelId, u32), Failure> {
        let number = self
            .store
            .next_number(hub.guild, &hub.room_name)
            .await
            .map_err(|error| {
                opening_failed(
                    "the next room number could not be read, so no room was opened",
                    &error,
                    scope,
                )
            })?;
        let spec = RoomSpec {
            name: room_channel_name(&hub.room_name, number),
            layout: layout.clone(),
        };
        let room = self
            .rooms
            .create_room(hub.guild, &spec)
            .await
            .map_err(|error| {
                opening_failed(
                    "a Join to Create room could not be opened",
                    &error,
                    &scope.clone().room_number(number),
                )
            })?;
        let scope = scope.clone().room(room).room_number(number);
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
            let failure = opening_failed(
                "a new room could not be recorded, so it was deleted",
                &error,
                &scope,
            );
            if let Err(cleanup) = self.rooms.delete_room(room).await {
                failure::report(
                    "voice.room.close_failed",
                    "an unrecorded room could not be deleted",
                    &Failure::from_error(&cleanup),
                    &scope,
                );
            }
            return Err(failure);
        }
        Ok((room, number))
    }

    async fn tell(&self, hub: &Hub, user: UserId, failure: &Failure, now_ms: u64, scope: &Scope) {
        if !self.may_notify(hub.channel, now_ms) {
            return;
        }
        let notice = room_notice(user, failure);
        if let Err(error) = self.rooms.notify(hub.channel, user, &notice).await {
            failure::report(
                "voice.notice.failed",
                "a member could not be told why their room did not open",
                &Failure::from_error(&error),
                scope,
            );
        }
    }

    fn may_notify(&self, hub: ChannelId, now_ms: u64) -> bool {
        let mut notices = self
            .notices
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let due = notices
            .get(&hub)
            .is_none_or(|last| now_ms.saturating_sub(*last) >= NOTICE_INTERVAL_MS);
        if due {
            notices.insert(hub, now_ms);
        }
        due
    }

    async fn abandon(&self, room: ChannelId, scope: &Scope) {
        match self.rooms.delete_room(room).await {
            Ok(RoomRemoval::Deleted | RoomRemoval::Gone) => {
                if let Err(error) = self.store.remove_room(room).await {
                    failure::report(
                        "voice.room.forgotten",
                        "an abandoned room could not be forgotten",
                        &Failure::from_error(&error),
                        scope,
                    );
                }
            }
            Err(error) => failure::report(
                "voice.room.close_failed",
                "an abandoned room could not be deleted, so the sweep will close it",
                &Failure::from_error(&error),
                scope,
            ),
        }
    }

    async fn close(&self, room: &Room, scope: &Scope) -> bool {
        match self.rooms.delete_room(room.channel).await {
            Ok(RoomRemoval::Deleted | RoomRemoval::Gone) => {
                match self.store.remove_room(room.channel).await {
                    Ok(_) => {
                        failure::record("voice.room.closed", "an empty room was closed", scope);
                        true
                    }
                    Err(error) => {
                        failure::report(
                            "voice.room.forgotten",
                            "a closed room could not be forgotten",
                            &Failure::from_error(&error),
                            scope,
                        );
                        false
                    }
                }
            }
            Err(error) => {
                failure::report(
                    "voice.room.close_failed",
                    "an empty room could not be deleted",
                    &Failure::from_error(&error),
                    scope,
                );
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

fn opening_failed(
    summary: &str,
    error: &(dyn std::error::Error + 'static),
    scope: &Scope,
) -> Failure {
    let failure = Failure::from_error(error);
    failure::report("voice.room.open_failed", summary, &failure, scope);
    failure
}

fn room_notice(user: UserId, failure: &Failure) -> String {
    format!(
        "<@{}> {}",
        user.get(),
        failure::failed("Barnacle could not open a room for you", failure)
    )
}

fn hub_scope(hub: &Hub) -> Scope {
    Scope::default().guild(hub.guild).hub(hub.channel)
}

fn room_scope(room: &Room) -> Scope {
    Scope::default()
        .guild(room.guild)
        .hub(room.hub)
        .room(room.channel)
        .room_number(room.number)
}

fn settled(created_at_ms: u64, now_ms: u64) -> bool {
    now_ms.saturating_sub(created_at_ms) >= NEW_CHANNEL_GRACE_MS
}

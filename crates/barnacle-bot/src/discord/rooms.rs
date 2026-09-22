use std::num::NonZeroU64;
use std::sync::Arc;

use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use crate::failure;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::RoleId;
use crate::voice;
use crate::voice::HubLayout;
use crate::voice::MoveOutcome;
use crate::voice::Occupancy;
use crate::voice::Overwrite;
use crate::voice::OverwriteTarget;
use crate::voice::RoomRemoval;
use crate::voice::RoomSpec;
use crate::voice::Rooms;
use crate::voice::RoomsError;
use crate::voice::with_room_access;

pub(super) const UNKNOWN_CHANNEL: isize = 10003;
pub(super) const MISSING_PERMISSIONS: isize = 50013;
const NOT_IN_VOICE: isize = 40032;

pub struct DiscordRooms {
    http: Arc<serenity::Http>,
    bot: serenity::UserId,
}

impl DiscordRooms {
    pub fn new(http: Arc<serenity::Http>, bot: serenity::UserId) -> Self {
        Self { http, bot }
    }
}

pub(super) fn refused(error: &serenity::Error, code: isize) -> bool {
    matches!(
        error,
        serenity::Error::Http(serenity::HttpError::UnsuccessfulRequest(response))
            if response.error.code == code
    )
}

fn failed(error: serenity::Error) -> RoomsError {
    RoomsError(Box::new(error))
}

#[derive(Debug, thiserror::Error)]
enum BadId {
    #[error("0 is not a server ID")]
    Guild,
    #[error("0 is not a channel ID")]
    Channel,
    #[error("0 is not a user ID")]
    User,
    #[error("0 is not a role ID")]
    Role,
}

fn guild_id(guild: GuildId) -> Result<serenity::GuildId, RoomsError> {
    NonZeroU64::new(guild.get())
        .map(serenity::GuildId::from)
        .ok_or_else(|| RoomsError(Box::new(BadId::Guild)))
}

fn channel_id(channel: ChannelId) -> Result<serenity::ChannelId, RoomsError> {
    NonZeroU64::new(channel.get())
        .map(serenity::ChannelId::from)
        .ok_or_else(|| RoomsError(Box::new(BadId::Channel)))
}

fn user_id(user: UserId) -> Result<serenity::UserId, RoomsError> {
    NonZeroU64::new(user.get())
        .map(serenity::UserId::from)
        .ok_or_else(|| RoomsError(Box::new(BadId::User)))
}

fn role_id(role: RoleId) -> Result<serenity::RoleId, RoomsError> {
    NonZeroU64::new(role.get())
        .map(serenity::RoleId::from)
        .ok_or_else(|| RoomsError(Box::new(BadId::Role)))
}

fn permission_overwrite(overwrite: Overwrite) -> Result<serenity::PermissionOverwrite, RoomsError> {
    let kind = match overwrite.target {
        OverwriteTarget::Role(role) => serenity::PermissionOverwriteType::Role(role_id(role)?),
        OverwriteTarget::Member(user) => serenity::PermissionOverwriteType::Member(user_id(user)?),
    };
    Ok(serenity::PermissionOverwrite {
        allow: serenity::Permissions::from_bits_truncate(overwrite.allow),
        deny: serenity::Permissions::from_bits_truncate(overwrite.deny),
        kind,
    })
}

fn overwrite(overwrite: &serenity::PermissionOverwrite) -> Option<Overwrite> {
    let target = match overwrite.kind {
        serenity::PermissionOverwriteType::Role(role) => {
            OverwriteTarget::Role(RoleId::new(role.get()))
        }
        serenity::PermissionOverwriteType::Member(user) => {
            OverwriteTarget::Member(UserId::new(user.get()))
        }
        _ => return None,
    };
    Some(Overwrite {
        target,
        allow: overwrite.allow.bits(),
        deny: overwrite.deny.bits(),
    })
}

impl Rooms for DiscordRooms {
    async fn create_room(&self, guild: GuildId, spec: &RoomSpec) -> Result<ChannelId, RoomsError> {
        let layout = &spec.layout;
        let permissions = with_room_access(&layout.overwrites, UserId::new(self.bot.get()))
            .into_iter()
            .map(permission_overwrite)
            .collect::<Result<Vec<_>, _>>()?;
        let category = layout.category.map(channel_id).transpose()?;
        let channel = serenity::CreateChannel::new(spec.name.as_str())
            .kind(serenity::ChannelType::Voice)
            .position(layout.position)
            .permissions(permissions);
        let channel = category
            .into_iter()
            .fold(channel, |channel, category| channel.category(category));
        let channel = layout
            .bitrate
            .into_iter()
            .fold(channel, |channel, bitrate| channel.bitrate(bitrate));
        let channel = layout
            .user_limit
            .into_iter()
            .fold(channel, |channel, limit| channel.user_limit(limit));
        let channel = layout.rtc_region.iter().fold(channel, |channel, region| {
            channel.rtc_region(region.clone())
        });
        let channel = layout
            .video_quality
            .into_iter()
            .fold(channel, |channel, mode| {
                channel.video_quality_mode(serenity::VideoQualityMode::from(mode))
            });
        let created = guild_id(guild)?
            .create_channel(&self.http, channel)
            .await
            .map_err(failed)?;
        Ok(ChannelId::new(created.id.get()))
    }

    async fn move_member(
        &self,
        guild: GuildId,
        user: UserId,
        room: ChannelId,
    ) -> Result<MoveOutcome, RoomsError> {
        match guild_id(guild)?
            .move_member(&self.http, user_id(user)?, channel_id(room)?)
            .await
        {
            Ok(_) => Ok(MoveOutcome::Moved),
            Err(error) if refused(&error, NOT_IN_VOICE) => Ok(MoveOutcome::NotInVoice),
            Err(error) => Err(failed(error)),
        }
    }

    async fn delete_room(&self, room: ChannelId) -> Result<RoomRemoval, RoomsError> {
        match channel_id(room)?.delete(&self.http).await {
            Ok(_) => Ok(RoomRemoval::Deleted),
            Err(error) if refused(&error, UNKNOWN_CHANNEL) => Ok(RoomRemoval::Gone),
            Err(error) => Err(failed(error)),
        }
    }

    async fn notify(&self, hub: ChannelId, user: UserId, content: &str) -> Result<(), RoomsError> {
        let message = serenity::CreateMessage::new()
            .content(content)
            .allowed_mentions(serenity::CreateAllowedMentions::new().users([user_id(user)?]));
        channel_id(hub)?
            .send_message(&self.http, message)
            .await
            .map_err(failed)?;
        Ok(())
    }
}

pub fn hub_layout(cache: &serenity::Cache, guild: GuildId, hub: ChannelId) -> Option<HubLayout> {
    let granted = failure::barnacle_permissions(cache, guild);
    let guild = cache.guild(NonZeroU64::new(guild.get())?)?;
    let channel = guild
        .channels
        .get(&serenity::ChannelId::from(NonZeroU64::new(hub.get())?))?;
    Some(HubLayout {
        category: channel
            .parent_id
            .map(|category| ChannelId::new(category.get())),
        position: channel.position,
        bitrate: channel.bitrate,
        user_limit: channel.user_limit,
        rtc_region: channel.rtc_region.clone(),
        video_quality: channel.video_quality_mode.map(u8::from),
        overwrites: channel
            .permission_overwrites
            .iter()
            .filter_map(overwrite)
            .collect(),
        blockers: granted
            .map(|granted| {
                failure::copy_blockers(
                    granted,
                    &channel.permission_overwrites,
                    serenity::Permissions::from_bits_truncate(voice::ROOM_ACCESS),
                )
            })
            .unwrap_or_default(),
    })
}

pub fn occupants(cache: &serenity::Cache, guild: GuildId, channel: ChannelId) -> usize {
    let Some(channel) = NonZeroU64::new(channel.get()).map(serenity::ChannelId::from) else {
        return 0;
    };
    NonZeroU64::new(guild.get())
        .and_then(|guild| cache.guild(guild))
        .map_or(0, |guild| voices_in(&guild, channel))
}

pub fn occupancy(cache: &serenity::Cache, guild: GuildId, channel: ChannelId) -> Occupancy {
    let Some(guild) = NonZeroU64::new(guild.get()).and_then(|guild| cache.guild(guild)) else {
        return Occupancy::Unknown;
    };
    let Some(channel) = NonZeroU64::new(channel.get())
        .map(serenity::ChannelId::from)
        .filter(|channel| guild.channels.contains_key(channel))
    else {
        return Occupancy::Missing;
    };
    match voices_in(&guild, channel) {
        0 => Occupancy::Empty,
        count => Occupancy::Occupied(count),
    }
}

fn voices_in(guild: &serenity::Guild, channel: serenity::ChannelId) -> usize {
    guild
        .voice_states
        .values()
        .filter(|state| state.channel_id == Some(channel))
        .count()
}

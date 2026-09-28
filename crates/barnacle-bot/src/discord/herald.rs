use std::sync::Arc;

use poise::serenity_prelude as serenity;

use super::board::allowed_mentions;
use crate::attendance::Delivery;
use crate::ids::ChannelId;
use crate::ids::Ping;
use crate::release_notes::Release;
use crate::updates::Herald;
use crate::updates::HeraldError;
use crate::wiring;

pub struct DiscordHerald {
    http: Arc<serenity::Http>,
}

impl DiscordHerald {
    pub fn new(http: Arc<serenity::Http>) -> Self {
        Self { http }
    }
}

pub(super) fn announcement_content(release: &Release, ping: Option<Ping>) -> String {
    match ping {
        Some(ping) => format!("{}\n{}", wiring::ping_content(&[ping]), release.render()),
        None => release.render(),
    }
}

pub fn announcement_message(release: &Release, ping: Option<Ping>) -> serenity::CreateMessage {
    let pings: Vec<Ping> = ping.into_iter().collect();
    serenity::CreateMessage::new()
        .content(announcement_content(release, ping))
        .allowed_mentions(allowed_mentions(wiring::ping_allowance(
            Delivery::New,
            &pings,
        )))
}

impl Herald for DiscordHerald {
    async fn post(
        &self,
        channel: ChannelId,
        release: &Release,
        ping: Option<Ping>,
    ) -> Result<(), HeraldError> {
        serenity::ChannelId::new(channel.get())
            .send_message(&self.http, announcement_message(release, ping))
            .await
            .map_err(|error| HeraldError(Box::new(error)))?;
        Ok(())
    }
}

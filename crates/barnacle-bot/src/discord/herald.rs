use std::sync::Arc;

use poise::serenity_prelude as serenity;

use super::board::allowed_mentions;
use crate::attendance::Delivery;
use crate::ids::ChannelId;
use crate::ids::Ping;
use crate::release_notes;
use crate::release_notes::Release;
use crate::text;
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

pub(super) fn release_embed(release: &Release) -> serenity::CreateEmbed {
    release.fields().into_iter().fold(
        serenity::CreateEmbed::new()
            .title(release.title())
            .colour(text::EMBED_COLOUR)
            .footer(serenity::CreateEmbedFooter::new(release_notes::FOOTER)),
        |embed, (name, value)| embed.field(name, value, false),
    )
}

impl Herald for DiscordHerald {
    async fn post(
        &self,
        channel: ChannelId,
        release: &Release,
        ping: Option<Ping>,
    ) -> Result<(), HeraldError> {
        let pings: Vec<Ping> = ping.into_iter().collect();
        let message = serenity::CreateMessage::new()
            .content(wiring::ping_content(&pings))
            .embed(release_embed(release))
            .allowed_mentions(allowed_mentions(wiring::ping_allowance(
                Delivery::New,
                &pings,
            )));
        serenity::ChannelId::new(channel.get())
            .send_message(&self.http, message)
            .await
            .map_err(|error| HeraldError(Box::new(error)))?;
        Ok(())
    }
}

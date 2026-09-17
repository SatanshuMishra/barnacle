use std::sync::Arc;

use barnacle_guess::Hint;
use barnacle_guess::Snowflake;
use poise::serenity_prelude as serenity;

use crate::ids::ChannelId;
use crate::table::AnnounceError;
use crate::table::Announcer;
use crate::table::Ending;
use crate::text;
use crate::wiring;

pub struct DiscordAnnouncer {
    http: Arc<serenity::Http>,
}

impl DiscordAnnouncer {
    pub fn new(http: Arc<serenity::Http>) -> Self {
        Self { http }
    }
}

pub fn cancel_row(custom_id: String, disabled: bool) -> serenity::CreateActionRow {
    serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(custom_id)
            .label(text::CANCEL_LABEL)
            .style(serenity::ButtonStyle::Secondary)
            .disabled(disabled),
    ])
}

impl Announcer for DiscordAnnouncer {
    async fn post_hint(&self, channel: ChannelId, hint: &Hint) -> Result<(), AnnounceError> {
        let message = serenity::CreateMessage::new().content(text::hint(hint));
        serenity::ChannelId::new(channel.get())
            .send_message(&self.http, message)
            .await
            .map_err(|error| AnnounceError(Box::new(error)))?;
        Ok(())
    }

    async fn post_ending(
        &self,
        channel: ChannelId,
        round_post: Snowflake,
        ending: &Ending,
    ) -> Result<(), AnnounceError> {
        let channel = serenity::ChannelId::new(channel.get());
        let message = match ending {
            Ending::Solved {
                solve,
                reveal,
                message,
                personal_best,
            } => serenity::CreateMessage::new()
                .content(text::win(reveal, solve.elapsed, *personal_best))
                .reference_message((channel, serenity::MessageId::new(message.get())))
                .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(true)),
            Ending::TimedOut { reveal } => {
                serenity::CreateMessage::new().content(text::timed_out(reveal))
            }
            Ending::Cancelled { by, reveal } => serenity::CreateMessage::new()
                .content(text::cancelled(*by, reveal))
                .allowed_mentions(serenity::CreateAllowedMentions::new()),
        };
        let disabled = serenity::EditMessage::new()
            .components(vec![cancel_row(wiring::ENDED_BUTTON_ID.to_owned(), true)]);
        let posted = channel.send_message(&self.http, message).await;
        let edited = channel
            .edit_message(
                &self.http,
                serenity::MessageId::new(round_post.get()),
                disabled,
            )
            .await;
        posted.map_err(|error| AnnounceError(Box::new(error)))?;
        edited.map_err(|error| AnnounceError(Box::new(error)))?;
        Ok(())
    }
}

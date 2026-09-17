use std::sync::Arc;

use barnacle_guess::Snowflake;
use poise::serenity_prelude as serenity;

use crate::attendance::Board;
use crate::attendance::BoardError;
use crate::attendance::PostTag;
use crate::attendance::Removal;
use crate::attendance::SignupView;
use crate::attendance::Target;
use crate::ids::ChannelId;
use crate::schedule::Hour;
use crate::text;
use crate::wiring;

const RECENT_MESSAGES: u8 = 50;
const UNKNOWN_MESSAGE: isize = 10008;
const UNKNOWN_CHANNEL: isize = 10003;

pub struct DiscordBoard {
    http: Arc<serenity::Http>,
    bot: serenity::UserId,
}

impl DiscordBoard {
    pub fn new(http: Arc<serenity::Http>, bot: serenity::UserId) -> Self {
        Self { http, bot }
    }
}

fn signup_embed(view: &SignupView) -> serenity::CreateEmbed {
    serenity::CreateEmbed::new()
        .title(text::signup_title(view.number, view.codename.as_deref()))
        .description(text::signup_description(view))
        .colour(text::EMBED_COLOUR)
}

fn signup_button(
    view: &SignupView,
    target: Target,
    attending: bool,
    label: String,
) -> serenity::CreateButton {
    let style = if attending {
        serenity::ButtonStyle::Success
    } else {
        serenity::ButtonStyle::Danger
    };
    serenity::CreateButton::new(wiring::signup_button_id(
        view.season,
        view.night,
        target,
        attending,
    ))
    .label(label)
    .style(style)
    .disabled(!view.open)
}

fn signup_rows(view: &SignupView) -> Vec<serenity::CreateActionRow> {
    let everything = serenity::CreateActionRow::Buttons(vec![
        signup_button(view, Target::All, true, text::ATTEND_ALL_LABEL.to_owned()),
        signup_button(view, Target::All, false, text::NOPE_ALL_LABEL.to_owned()),
    ]);
    std::iter::once(everything)
        .chain(Hour::ALL.into_iter().map(|hour| {
            serenity::CreateActionRow::Buttons(vec![
                signup_button(
                    view,
                    Target::One(hour),
                    true,
                    text::hour_button_label(hour, true),
                ),
                signup_button(
                    view,
                    Target::One(hour),
                    false,
                    text::hour_button_label(hour, false),
                ),
            ])
        }))
        .collect()
}

fn tagged(message: &serenity::Message, prefix: &str) -> bool {
    message
        .components
        .iter()
        .flat_map(|row| &row.components)
        .any(|component| match component {
            serenity::ActionRowComponent::Button(button) => match &button.data {
                serenity::ButtonKind::NonLink { custom_id, .. } => custom_id.starts_with(prefix),
                _ => false,
            },
            _ => false,
        })
}

fn vanished(error: &serenity::Error) -> bool {
    matches!(
        error,
        serenity::Error::Http(serenity::HttpError::UnsuccessfulRequest(response))
            if response.status_code == serenity::StatusCode::NOT_FOUND
                && matches!(response.error.code, UNKNOWN_MESSAGE | UNKNOWN_CHANNEL)
    )
}

fn failed(error: serenity::Error) -> BoardError {
    BoardError(Box::new(error))
}

impl Board for DiscordBoard {
    async fn send_post(
        &self,
        channel: ChannelId,
        view: &SignupView,
    ) -> Result<Snowflake, BoardError> {
        let message = serenity::CreateMessage::new()
            .embed(signup_embed(view))
            .components(signup_rows(view))
            .allowed_mentions(serenity::CreateAllowedMentions::new());
        let posted = serenity::ChannelId::new(channel.get())
            .send_message(&self.http, message)
            .await
            .map_err(failed)?;
        Ok(Snowflake::new(posted.id.get()))
    }

    async fn find_post(
        &self,
        channel: ChannelId,
        tag: PostTag,
    ) -> Result<Option<Snowflake>, BoardError> {
        let prefix = wiring::signup_tag_prefix(tag.season, tag.night);
        let recent = serenity::ChannelId::new(channel.get())
            .messages(
                &self.http,
                serenity::GetMessages::new().limit(RECENT_MESSAGES),
            )
            .await
            .map_err(failed)?;
        Ok(recent
            .into_iter()
            .find(|message| message.author.id == self.bot && tagged(message, &prefix))
            .map(|message| Snowflake::new(message.id.get())))
    }

    async fn edit_post(
        &self,
        channel: ChannelId,
        message: Snowflake,
        view: &SignupView,
    ) -> Result<(), BoardError> {
        let edit = serenity::EditMessage::new()
            .embed(signup_embed(view))
            .components(signup_rows(view))
            .allowed_mentions(serenity::CreateAllowedMentions::new());
        serenity::ChannelId::new(channel.get())
            .edit_message(&self.http, serenity::MessageId::new(message.get()), edit)
            .await
            .map_err(failed)?;
        Ok(())
    }

    async fn delete_post(
        &self,
        channel: ChannelId,
        message: Snowflake,
    ) -> Result<Removal, BoardError> {
        match serenity::ChannelId::new(channel.get())
            .delete_message(&self.http, serenity::MessageId::new(message.get()))
            .await
        {
            Ok(()) => Ok(Removal::Deleted),
            Err(error) if vanished(&error) => Ok(Removal::Gone),
            Err(error) => Err(failed(error)),
        }
    }
}

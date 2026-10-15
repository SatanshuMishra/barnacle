use std::sync::Arc;

use barnacle_guess::Snowflake;
use poise::serenity_prelude as serenity;

use crate::attendance::Board;
use crate::attendance::BoardError;
use crate::attendance::Delivery;
use crate::attendance::PostTag;
use crate::attendance::Removal;
use crate::attendance::Sent;
use crate::attendance::SignupView;
use crate::attendance::Target;
use crate::ids::ChannelId;
use crate::ids::RoleId;
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
        .title(text::signup_title(&view.name))
        .description(text::signup_description(view))
        .colour(text::EMBED_COLOUR)
}

fn ping_mentions(delivery: Delivery, view: &SignupView) -> serenity::CreateAllowedMentions {
    allowed_mentions(wiring::ping_allowance(delivery, &view.pings))
}

pub fn allowed_mentions(allowance: wiring::PingAllowance) -> serenity::CreateAllowedMentions {
    let mentions = serenity::CreateAllowedMentions::new().roles(
        allowance
            .roles
            .into_iter()
            .filter_map(|role| std::num::NonZeroU64::new(role.get()))
            .map(serenity::RoleId::from),
    );
    if allowance.everyone {
        mentions.everyone(true)
    } else {
        mentions
    }
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
        view.event,
        view.session.date(),
        target,
        attending,
    ))
    .label(label)
    .style(style)
    .disabled(!view.open)
}

pub fn signup_message(view: &SignupView, delivery: Delivery) -> serenity::CreateMessage {
    serenity::CreateMessage::new()
        .content(wiring::ping_content(&view.pings))
        .embed(signup_embed(view))
        .components(signup_rows(view))
        .allowed_mentions(ping_mentions(delivery, view))
}

pub fn signup_edit(view: &SignupView) -> serenity::EditMessage {
    serenity::EditMessage::new()
        .content(wiring::ping_content(&view.pings))
        .embed(signup_embed(view))
        .components(signup_rows(view))
        .allowed_mentions(ping_mentions(Delivery::Redraw, view))
}

pub fn signup_rows(view: &SignupView) -> Vec<serenity::CreateActionRow> {
    let hours = view.session.hours();
    let choose = (!text::short_event(hours)).then(|| {
        serenity::CreateButton::new(wiring::choose_button_id(view.event, view.session.date()))
            .label(text::CHOOSE_HOURS_LABEL)
            .style(serenity::ButtonStyle::Secondary)
            .disabled(!view.open)
    });
    let everything = serenity::CreateActionRow::Buttons(
        [
            signup_button(view, Target::All, true, text::ATTEND_ALL_LABEL.to_owned()),
            signup_button(view, Target::All, false, text::NOPE_ALL_LABEL.to_owned()),
        ]
        .into_iter()
        .chain(choose)
        .collect(),
    );
    std::iter::once(everything)
        .chain(
            hours
                .each()
                .filter(|_| text::short_event(hours))
                .map(|hour| {
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
                }),
        )
        .collect()
}

pub fn hours_menu_reply(
    view: &SignupView,
    attending: &[Hour],
) -> serenity::CreateInteractionResponseMessage {
    let hours = view.session.hours();
    let options = hours
        .each()
        .map(|hour| {
            serenity::CreateSelectMenuOption::new(
                format!("Hour {}", hour.get()),
                hour.get().to_string(),
            )
            .default_selection(attending.contains(&hour))
        })
        .collect();
    let menu = serenity::CreateSelectMenu::new(
        wiring::hours_menu_id(view.event, view.session.date()),
        serenity::CreateSelectMenuKind::String { options },
    )
    .min_values(0)
    .max_values(hours.get());
    serenity::CreateInteractionResponseMessage::new()
        .ephemeral(true)
        .allowed_mentions(serenity::CreateAllowedMentions::new())
        .content(text::hours_menu_text(view))
        .components(vec![serenity::CreateActionRow::SelectMenu(menu)])
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

fn channel_id(channel: ChannelId) -> Result<serenity::ChannelId, BoardError> {
    std::num::NonZeroU64::new(channel.get())
        .map(serenity::ChannelId::from)
        .ok_or_else(|| BoardError(Box::new(BadId::Channel)))
}

fn message_id(message: Snowflake) -> Result<serenity::MessageId, BoardError> {
    std::num::NonZeroU64::new(message.get())
        .map(serenity::MessageId::from)
        .ok_or_else(|| BoardError(Box::new(BadId::Message)))
}

#[derive(Debug, thiserror::Error)]
enum BadId {
    #[error("the attendance database holds 0 as a channel ID")]
    Channel,
    #[error("the attendance database holds 0 as a message ID")]
    Message,
}

impl Board for DiscordBoard {
    async fn send_post(
        &self,
        channel: ChannelId,
        view: &SignupView,
        delivery: Delivery,
    ) -> Result<Sent, BoardError> {
        let posted = channel_id(channel)?
            .send_message(&self.http, signup_message(view, delivery))
            .await
            .map_err(failed)?;
        let mentioned: Vec<RoleId> = posted
            .mention_roles
            .iter()
            .map(|role| RoleId::new(role.get()))
            .collect();
        Ok(Sent {
            message: Snowflake::new(posted.id.get()),
            unheard: wiring::unheard_pings(&view.pings, &mentioned, posted.mention_everyone),
        })
    }

    async fn find_post(
        &self,
        channel: ChannelId,
        tag: PostTag,
    ) -> Result<Option<Snowflake>, BoardError> {
        let prefix = wiring::signup_tag_prefix(tag.event, tag.session.date());
        let recent = channel_id(channel)?
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
        channel_id(channel)?
            .edit_message(&self.http, message_id(message)?, signup_edit(view))
            .await
            .map_err(failed)?;
        Ok(())
    }

    async fn delete_post(
        &self,
        channel: ChannelId,
        message: Snowflake,
    ) -> Result<Removal, BoardError> {
        match channel_id(channel)?
            .delete_message(&self.http, message_id(message)?)
            .await
        {
            Ok(()) => Ok(Removal::Deleted),
            Err(error) if vanished(&error) => Ok(Removal::Gone),
            Err(error) => Err(failed(error)),
        }
    }
}

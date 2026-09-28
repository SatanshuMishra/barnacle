use std::sync::Arc;

use barnacle_catalog::ShipIndex;
use barnacle_catalog::store::CatalogRoot;
use barnacle_guess::Draw;
use barnacle_guess::Hint;
use barnacle_guess::SeriesLength;
use barnacle_guess::Snowflake;
use barnacle_guess::Timing;
use poise::serenity_prelude as serenity;

use crate::failure;
use crate::failure::Failure;
use crate::failure::Scope;
use crate::ids::ChannelId;
use crate::table::AnnounceError;
use crate::table::Announcer;
use crate::table::Ending;
use crate::text;
use crate::wiring;

pub(super) const SILHOUETTE_FILE: &str = "silhouette.png";

pub struct DiscordAnnouncer {
    http: Arc<serenity::Http>,
    root: CatalogRoot,
    catalog_name: String,
}

impl DiscordAnnouncer {
    pub fn new(http: Arc<serenity::Http>, root: CatalogRoot, catalog_name: String) -> Self {
        Self {
            http,
            root,
            catalog_name,
        }
    }

    async fn send_and_disable(
        &self,
        channel: serenity::ChannelId,
        message: serenity::CreateMessage,
        round_post: Snowflake,
        disabled: serenity::CreateActionRow,
    ) -> (
        Result<serenity::Message, AnnounceError>,
        Result<(), AnnounceError>,
    ) {
        let disabled = serenity::EditMessage::new().components(vec![disabled]);
        let posted = channel.send_message(&self.http, message).await;
        let edited = channel
            .edit_message(
                &self.http,
                serenity::MessageId::new(round_post.get()),
                disabled,
            )
            .await;
        (
            posted.map_err(announce_error),
            edited.map(|_| ()).map_err(announce_error),
        )
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

pub fn ending_mentions(winner_reply: bool) -> serenity::CreateAllowedMentions {
    if winner_reply {
        serenity::CreateAllowedMentions::new().replied_user(true)
    } else {
        serenity::CreateAllowedMentions::new()
    }
}

pub(super) fn round_embed(draw: &Draw) -> serenity::CreateEmbed {
    let embed = serenity::CreateEmbed::new()
        .title(text::ROUND_TITLE)
        .description(text::ROUND_DESCRIPTION)
        .colour(text::EMBED_COLOUR)
        .field(text::TIERS_FIELD, text::tier_range(draw.options()), true)
        .image(format!("attachment://{SILHOUETTE_FILE}"))
        .footer(serenity::CreateEmbedFooter::new(text::round_footer(
            Timing::STANDARD,
        )));
    if draw.options().historical() {
        embed.field(text::PAPER_EXCLUDED, "Yes", true)
    } else {
        embed
    }
}

pub(super) async fn silhouette_attachment(
    root: &CatalogRoot,
    catalog_name: &str,
    ship: &ShipIndex,
) -> std::io::Result<serenity::CreateAttachment> {
    let png = tokio::fs::read(root.silhouette(catalog_name, ship)).await?;
    Ok(serenity::CreateAttachment::bytes(png, SILHOUETTE_FILE))
}

fn series_row(number: Option<u64>) -> serenity::CreateActionRow {
    let (skip, end) = number.map_or_else(
        || {
            (
                wiring::ENDED_SKIP_BUTTON_ID.to_owned(),
                wiring::ENDED_END_BUTTON_ID.to_owned(),
            )
        },
        |number| {
            (
                wiring::skip_button_id(number),
                wiring::end_series_button_id(number),
            )
        },
    );
    serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(skip)
            .label(text::SKIP_LABEL)
            .style(serenity::ButtonStyle::Secondary)
            .disabled(number.is_none()),
        serenity::CreateButton::new(end)
            .label(text::END_SERIES_LABEL)
            .style(serenity::ButtonStyle::Danger)
            .disabled(number.is_none()),
    ])
}

fn still_playing_row(series: u64) -> serenity::CreateActionRow {
    serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(wiring::still_playing_button_id(series))
            .label(text::STILL_PLAYING_LABEL)
            .style(serenity::ButtonStyle::Primary),
    ])
}

pub fn ending_message(
    channel: serenity::ChannelId,
    content: impl Into<String>,
    reply_to: Option<Snowflake>,
) -> serenity::CreateMessage {
    let message = serenity::CreateMessage::new()
        .content(content)
        .allowed_mentions(ending_mentions(reply_to.is_some()));
    match reply_to {
        Some(winner) => message.reference_message(
            serenity::MessageReference::new(serenity::MessageReferenceKind::Default, channel)
                .message_id(serenity::MessageId::new(winner.get()))
                .fail_if_not_exists(false),
        ),
        None => message,
    }
}

pub fn series_edit(content: &str) -> serenity::EditMessage {
    serenity::EditMessage::new()
        .content(content)
        .components(Vec::new())
        .allowed_mentions(serenity::CreateAllowedMentions::new())
}

fn announce_error(error: serenity::Error) -> AnnounceError {
    AnnounceError(Box::new(error))
}

impl Announcer for DiscordAnnouncer {
    async fn post_hint(&self, channel: ChannelId, hint: &Hint) -> Result<(), AnnounceError> {
        let message = serenity::CreateMessage::new()
            .content(text::hint(hint))
            .allowed_mentions(serenity::CreateAllowedMentions::new());
        serenity::ChannelId::new(channel.get())
            .send_message(&self.http, message)
            .await
            .map_err(announce_error)?;
        Ok(())
    }

    async fn post_ending(
        &self,
        channel: ChannelId,
        round_post: Snowflake,
        ending: &Ending,
    ) -> Result<(), AnnounceError> {
        let channel = serenity::ChannelId::new(channel.get());
        let message = ending_message(
            channel,
            text::ending_result(ending),
            ending.winning_message(),
        );
        let (posted, edited) = self
            .send_and_disable(
                channel,
                message,
                round_post,
                cancel_row(wiring::ENDED_BUTTON_ID.to_owned(), true),
            )
            .await;
        posted?;
        edited?;
        Ok(())
    }

    async fn post_series_round(
        &self,
        channel: ChannelId,
        draw: &Draw,
        number: u64,
        round: u32,
        length: SeriesLength,
    ) -> Result<Snowflake, AnnounceError> {
        let attachment = silhouette_attachment(&self.root, &self.catalog_name, draw.ship())
            .await
            .map_err(|error| AnnounceError(Box::new(error)))?;
        let message = serenity::CreateMessage::new()
            .embed(round_embed(draw).field(
                text::SERIES_FIELD,
                text::series_round_label(round, length),
                true,
            ))
            .add_file(attachment)
            .components(vec![series_row(Some(number))])
            .allowed_mentions(serenity::CreateAllowedMentions::new());
        let posted = serenity::ChannelId::new(channel.get())
            .send_message(&self.http, message)
            .await
            .map_err(announce_error)?;
        Ok(Snowflake::new(posted.id.get()))
    }

    async fn post_series_ending(
        &self,
        channel: ChannelId,
        round_post: Snowflake,
        content: &str,
        reply_to: Option<Snowflake>,
    ) -> Result<Snowflake, AnnounceError> {
        let scope = Scope::default().channel(channel).message(round_post);
        let channel = serenity::ChannelId::new(channel.get());
        let message = ending_message(channel, content, reply_to);
        let (posted, edited) = self
            .send_and_disable(channel, message, round_post, series_row(None))
            .await;
        let posted = posted?;
        if let Err(error) = edited {
            failure::report(
                "guess.post.failed",
                "a series round's buttons could not be turned off",
                &Failure::from_error(&error),
                &scope,
            );
        }
        Ok(Snowflake::new(posted.id.get()))
    }

    async fn post_series_message(
        &self,
        channel: ChannelId,
        content: &str,
        still_playing: Option<u64>,
    ) -> Result<Snowflake, AnnounceError> {
        let message = serenity::CreateMessage::new()
            .content(content)
            .allowed_mentions(serenity::CreateAllowedMentions::new());
        let message = match still_playing {
            Some(series) => message.components(vec![still_playing_row(series)]),
            None => message,
        };
        let posted = serenity::ChannelId::new(channel.get())
            .send_message(&self.http, message)
            .await
            .map_err(announce_error)?;
        Ok(Snowflake::new(posted.id.get()))
    }

    async fn edit_series_message(
        &self,
        channel: ChannelId,
        message: Snowflake,
        content: &str,
    ) -> Result<(), AnnounceError> {
        serenity::ChannelId::new(channel.get())
            .edit_message(
                &self.http,
                serenity::MessageId::new(message.get()),
                series_edit(content),
            )
            .await
            .map_err(announce_error)?;
        Ok(())
    }

    async fn delete_series_message(
        &self,
        channel: ChannelId,
        message: Snowflake,
    ) -> Result<(), AnnounceError> {
        serenity::ChannelId::new(channel.get())
            .delete_message(&self.http, serenity::MessageId::new(message.get()))
            .await
            .map_err(announce_error)
    }
}

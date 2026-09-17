use barnacle_guess::Guess;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use super::Data;
use super::Error;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Place;
use crate::table::CancelOutcome;
use crate::text;
use crate::wiring;

pub async fn handle(
    framework: poise::FrameworkContext<'_, Data, Error>,
    event: &serenity::FullEvent,
) -> Result<(), Error> {
    let data = framework.user_data;
    match event {
        serenity::FullEvent::Message { new_message } => {
            let Some(guild) = new_message.guild_id else {
                return Ok(());
            };
            let place = Place {
                guild: GuildId::new(guild.get()),
                channel: ChannelId::new(new_message.channel_id.get()),
            };
            let guess = Guess {
                author: UserId::new(new_message.author.id.get()),
                author_is_bot: new_message.author.bot,
                message: Snowflake::new(new_message.id.get()),
                text: &new_message.content,
            };
            data.table.hear(place, guess).await;
            Ok(())
        }
        serenity::FullEvent::InteractionCreate {
            interaction: serenity::Interaction::Component(component),
        } => {
            let (Some(number), Some(guild)) = (
                wiring::round_number(&component.data.custom_id),
                component.guild_id,
            ) else {
                return Ok(());
            };
            let place = Place {
                guild: GuildId::new(guild.get()),
                channel: ChannelId::new(component.channel_id.get()),
            };
            let can_manage_messages = component
                .member
                .as_ref()
                .and_then(|member| member.permissions)
                .is_some_and(|permissions| permissions.manage_messages());
            let serenity_context = framework.serenity_context;
            component.defer_ephemeral(serenity_context).await?;
            let outcome = data
                .table
                .cancel(
                    place,
                    number,
                    UserId::new(component.user.id.get()),
                    can_manage_messages,
                )
                .await;
            match outcome {
                CancelOutcome::Cancelled => component.delete_response(serenity_context).await?,
                CancelOutcome::Refused => {
                    private_edit(serenity_context, component, text::CANCEL_REFUSED).await?;
                }
                CancelOutcome::AlreadyOver => {
                    private_edit(serenity_context, component, text::ROUND_OVER).await?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

async fn private_edit(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    content: &str,
) -> Result<(), serenity::Error> {
    component
        .edit_response(
            serenity_context,
            serenity::EditInteractionResponse::new().content(content),
        )
        .await?;
    Ok(())
}

pub async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Command { error, ctx, .. } => {
            tracing::error!(command = %ctx.command().qualified_name, %error, "a command failed");
            let reply = poise::CreateReply::default()
                .content(text::SOMETHING_WENT_WRONG)
                .ephemeral(true);
            if let Err(error) = ctx.send(reply).await {
                tracing::error!(%error, "the failure notice could not be sent");
            }
        }
        other => {
            if let Err(error) = poise::builtins::on_error(other).await {
                tracing::error!(%error, "an error could not be handled");
            }
        }
    }
}

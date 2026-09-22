use barnacle_guess::Guess;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use super::Context;
use super::Data;
use super::Error;
use super::rooms;
use crate::attendance::Click;
use crate::attendance::ClickOutcome;
use crate::failure;
use crate::failure::Failure;
use crate::failure::Kind;
use crate::failure::Scope;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Place;
use crate::table::CancelOutcome;
use crate::text;
use crate::wiring;
use crate::wiring::SignupClick;

const BUTTON_FAILED: &str = "That button did not work";
const SIGNUP_FAILED: &str = "Your sign-up was not saved";
const COMMANDS_OUT_OF_DATE: &str = "Discord's copy of Barnacle's commands is out of date; it refreshes the next time Barnacle starts.";

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
            clicked(data, framework.serenity_context, component).await;
            Ok(())
        }
        serenity::FullEvent::VoiceStateUpdate { old, new } => {
            voice_moved(data, framework.serenity_context, old.as_ref(), new).await;
            Ok(())
        }
        serenity::FullEvent::ChannelDelete { channel, .. } => {
            data.voice
                .channel_deleted(ChannelId::new(channel.id.get()))
                .await;
            Ok(())
        }
        _ => Ok(()),
    }
}

async fn clicked(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
) {
    let Some(guild) = component.guild_id.map(|guild| GuildId::new(guild.get())) else {
        return;
    };
    if let Some(signup) = wiring::signup_click(&component.data.custom_id) {
        sign_up(data, serenity_context, component, signup, guild).await;
    } else if let Some(number) = wiring::round_number(&component.data.custom_id) {
        cancel(data, serenity_context, component, number, guild).await;
    }
}

async fn cancel(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    number: u64,
    guild: GuildId,
) {
    let scope = Scope {
        round: u32::try_from(number).ok(),
        ..Scope::of_component(component).guild(guild)
    };
    let place = Place {
        guild,
        channel: ChannelId::new(component.channel_id.get()),
    };
    let can_manage_messages = component
        .member
        .as_ref()
        .and_then(|member| member.permissions)
        .is_some_and(|permissions| permissions.manage_messages());
    if !acknowledge(serenity_context, component, &scope).await {
        return;
    }
    let outcome = data
        .table
        .cancel(
            place,
            number,
            UserId::new(component.user.id.get()),
            can_manage_messages,
        )
        .await;
    let (reason, content) = match outcome {
        CancelOutcome::Cancelled => return,
        CancelOutcome::Refused => ("not_your_round", text::CANCEL_REFUSED),
        CancelOutcome::AlreadyOver => ("round_over", text::ROUND_OVER),
    };
    failure::refused(
        "interaction.refused",
        "a cancel click was turned down",
        reason,
        &scope,
    );
    tell(
        serenity_context,
        component,
        Answer::Followup,
        content,
        &scope,
    )
    .await;
}

async fn voice_moved(
    data: &Data,
    serenity_context: &serenity::Context,
    old: Option<&serenity::VoiceState>,
    new: &serenity::VoiceState,
) {
    let Some(guild) = new.guild_id else {
        return;
    };
    let guild = GuildId::new(guild.get());
    let user = UserId::new(new.user_id.get());
    let left = old
        .and_then(|state| state.channel_id)
        .map(|channel| ChannelId::new(channel.get()));
    let joined = new.channel_id.map(|channel| ChannelId::new(channel.get()));
    if left == joined {
        return;
    }
    let scope = Scope::default().guild(guild).user(user);
    let Some(now_ms) = super::now_ms() else {
        failure::report(
            "voice.state.failed",
            "a voice change was skipped because the clock reads before 1970",
            &Failure::internal("the clock reads before 1970"),
            &scope,
        );
        return;
    };
    if let Some(left) = left {
        let occupants = rooms::occupants(&serenity_context.cache, guild, left);
        data.voice.left(left, occupants, now_ms).await;
    }
    let Some(joined) = joined else {
        return;
    };
    let hub = match data.voice.store().hub(joined).await {
        Ok(Some(hub)) => hub,
        Ok(None) => {
            data.voice.entered(joined).await;
            return;
        }
        Err(error) => {
            failure::report(
                "voice.state.failed",
                "a joined voice channel could not be looked up",
                &Failure::from_error(&error),
                &scope.channel(joined),
            );
            return;
        }
    };
    if new.member.as_ref().is_some_and(|member| member.user.bot) {
        return;
    }
    let Some(layout) = rooms::hub_layout(&serenity_context.cache, hub.guild, hub.channel) else {
        failure::report(
            "voice.room.open_failed",
            "a Join to Create channel is not in the cache, so no room was opened",
            &Failure::internal("the Join to Create channel is not in Barnacle's cache"),
            &scope.hub(hub.channel),
        );
        return;
    };
    data.voice.joined(&hub, user, &layout, now_ms).await;
}

async fn sign_up(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    signup: SignupClick,
    guild: GuildId,
) {
    let scope = Scope::of_component(component)
        .guild(guild)
        .season(signup.season)
        .night(signup.night.label());
    let now_unix = super::now_unix();
    let start_unix = signup.night.start_unix();
    if start_unix <= now_unix {
        failure::refused(
            "interaction.refused",
            "a sign-up click came after sign-ups closed",
            "signups_closed",
            &scope,
        );
        tell(
            serenity_context,
            component,
            Answer::Response,
            &text::signups_closed_at(start_unix),
            &scope,
        )
        .await;
        return;
    }
    if !acknowledge(serenity_context, component, &scope).await {
        return;
    }
    let Some(now_ms) = super::now_ms() else {
        let failure = Failure::internal("the clock reads before 1970");
        failure::report(
            "interaction.failed",
            "a sign-up could not be saved because the clock reads before 1970",
            &failure,
            &scope,
        );
        tell(
            serenity_context,
            component,
            Answer::Followup,
            &failure::failed(SIGNUP_FAILED, &failure),
            &scope,
        )
        .await;
        return;
    };
    let click = Click {
        guild,
        channel: ChannelId::new(component.channel_id.get()),
        message: Snowflake::new(component.message.id.get()),
        user: UserId::new(component.user.id.get()),
        season: signup.season,
        night: signup.night,
        target: signup.target,
        attending: signup.attending,
    };
    let content = match data.signups.click(click, now_unix, now_ms).await {
        ClickOutcome::Recorded => return,
        ClickOutcome::Closed { start_unix } => {
            failure::refused(
                "interaction.refused",
                "a sign-up click came after sign-ups closed",
                "signups_closed",
                &scope,
            );
            text::signups_closed_at(start_unix)
        }
        ClickOutcome::UnknownSeason => {
            failure::refused(
                "interaction.refused",
                "a sign-up click named a season that is gone",
                "season_gone",
                &scope,
            );
            text::SEASON_GONE.to_owned()
        }
        ClickOutcome::Failed(failure) => {
            failure::report(
                "interaction.failed",
                "a sign-up could not be saved",
                &failure,
                &scope,
            );
            failure::failed(SIGNUP_FAILED, &failure)
        }
    };
    tell(
        serenity_context,
        component,
        Answer::Followup,
        &content,
        &scope,
    )
    .await;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    Response,
    Followup,
}

async fn acknowledge(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    scope: &Scope,
) -> bool {
    let Err(error) = component.defer(serenity_context).await else {
        return true;
    };
    let failure = Failure::from_error(&error);
    failure::report(
        "interaction.failed",
        "a button click could not be acknowledged",
        &failure,
        scope,
    );
    if failure.kind != Kind::UnknownInteraction {
        tell(
            serenity_context,
            component,
            Answer::Response,
            &failure::failed(BUTTON_FAILED, &failure),
            scope,
        )
        .await;
    }
    false
}

async fn tell(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    answer: Answer,
    content: &str,
    scope: &Scope,
) {
    let sent = match answer {
        Answer::Response => {
            component
                .create_response(
                    serenity_context,
                    serenity::CreateInteractionResponse::Message(
                        serenity::CreateInteractionResponseMessage::new()
                            .content(content)
                            .ephemeral(true),
                    ),
                )
                .await
        }
        Answer::Followup => component
            .create_followup(
                serenity_context,
                serenity::CreateInteractionResponseFollowup::new()
                    .content(content)
                    .ephemeral(true),
            )
            .await
            .map(|_| ()),
    };
    if let Err(error) = sent {
        failure::report(
            "interaction.failed",
            "the answer to a button click could not be sent",
            &Failure::from_error(&error),
            scope,
        );
    }
}

pub async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Command { error, ctx, .. } => {
            let scope = Scope::of_command(ctx);
            let failure = Failure::from_error(&*error);
            failure::report(
                "command.failed",
                "a command did not finish",
                &failure,
                &scope,
            );
            let content = failure::command_failed(&ctx.command().qualified_name, &failure);
            reply(ctx, content, &scope).await;
        }
        poise::FrameworkError::MissingUserPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            let scope = Scope::of_command(ctx);
            failure::refused(
                "command.refused",
                "a member lacks the permissions a command needs",
                "missing_user_permissions",
                &scope,
            );
            let command = &ctx.command().qualified_name;
            let names = missing_permissions
                .map(|missing| missing.get_permission_names())
                .unwrap_or_default();
            let content = if names.is_empty() {
                format!("You need a permission you do not have to use `/{command}`.")
            } else {
                format!(
                    "You need {} to use `/{command}`.",
                    failure::join_names(&names)
                )
            };
            reply(ctx, content, &scope).await;
        }
        poise::FrameworkError::MissingBotPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            let scope = Scope::of_command(ctx);
            failure::refused(
                "command.refused",
                "Barnacle lacks the permissions a command needs",
                "missing_bot_permissions",
                &scope,
            );
            let names = missing_permissions.get_permission_names();
            let content = format!(
                "Barnacle needs {} here to run `/{}`. A server admin can grant {} to Barnacle's role, then try again.",
                failure::join_names(&names),
                ctx.command().qualified_name,
                if names.len() == 1 { "it" } else { "them" }
            );
            reply(ctx, content, &scope).await;
        }
        poise::FrameworkError::GuildOnly { ctx, .. } => {
            let scope = Scope::of_command(ctx);
            failure::refused(
                "command.refused",
                "a server-only command was used outside a server",
                "guild_only",
                &scope,
            );
            let content = format!(
                "`/{}` works only inside a server.",
                ctx.command().qualified_name
            );
            reply(ctx, content, &scope).await;
        }
        poise::FrameworkError::ArgumentParse { error, ctx, .. } => {
            let scope = Scope::of_command(ctx);
            failure::refused(
                "command.refused",
                "a command option could not be read",
                "invalid_option",
                &scope,
            );
            let content = format!(
                "Barnacle could not read one of the options you gave `/{}`: {error}. Check it and try again.",
                ctx.command().qualified_name
            );
            reply(ctx, content, &scope).await;
        }
        poise::FrameworkError::CommandStructureMismatch {
            description, ctx, ..
        } => {
            let ctx = poise::Context::Application(ctx);
            let scope = Scope::of_command(ctx);
            let command = &ctx.command().qualified_name;
            let failure = Failure::internal(format!(
                "Discord sent options that do not match `/{command}`: {description}"
            ));
            failure::report(
                "command.failed",
                "Discord's copy of a command does not match Barnacle's",
                &failure,
                &scope,
            );
            reply(ctx, out_of_date(command, &failure), &scope).await;
        }
        poise::FrameworkError::EventHandler { error, event, .. } => {
            failure::report(
                "event.failed",
                &format!("the handler for a {} event failed", event.snake_case_name()),
                &Failure::from_error(&*error),
                &Scope::default(),
            );
        }
        poise::FrameworkError::Setup { error, .. } => {
            failure::report(
                "service.failed",
                "Barnacle could not finish setting up after connecting to Discord",
                &Failure::from_error(&*error),
                &Scope::default(),
            );
        }
        other => {
            let scope = other.ctx().map(Scope::of_command).unwrap_or_default();
            failure::refused(
                "command.refused",
                "the framework turned a command down",
                "framework",
                &scope,
            );
            if let Err(error) = poise::builtins::on_error(other).await {
                failure::report(
                    "interaction.reply_failed",
                    "the framework's reply could not be sent",
                    &Failure::from_error(&error),
                    &scope,
                );
            }
        }
    }
}

fn out_of_date(command: &str, failure: &Failure) -> String {
    let reference = format!("(Reference: {})", failure.reference);
    let message = failure::command_failed(command, failure);
    match message.strip_suffix(&reference) {
        Some(explained) => format!("{explained}{COMMANDS_OUT_OF_DATE} {reference}"),
        None => format!("{message} {COMMANDS_OUT_OF_DATE}"),
    }
}

async fn reply(ctx: Context<'_>, content: String, scope: &Scope) {
    let reply = poise::CreateReply::default()
        .content(content)
        .ephemeral(true);
    if let Err(error) = ctx.send(reply).await {
        failure::report(
            "interaction.reply_failed",
            "a reply to a command could not be sent",
            &Failure::from_error(&error),
            scope,
        );
    }
}

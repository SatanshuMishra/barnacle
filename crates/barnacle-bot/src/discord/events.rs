use std::collections::HashSet;

use barnacle_guess::Guess;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use super::Context;
use super::Data;
use super::Error;
use super::board;
use super::panel;
use super::rooms;
use crate::attendance::ChooseOutcome;
use crate::attendance::Click;
use crate::attendance::ClickOutcome;
use crate::attendance::HoursPick;
use crate::attendance::MenuOutcome;
use crate::failure;
use crate::failure::Failure;
use crate::failure::Kind;
use crate::failure::OptionProblem;
use crate::failure::Scope;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Place;
use crate::table::CancelOutcome;
use crate::text;
use crate::updates::SendOutcome;
use crate::wiring;
use crate::wiring::SeriesClick;
use crate::wiring::SessionRef;
use crate::wiring::SignupClick;

const BUTTON_FAILED: &str = "That button did not work";
const SIGNUP_FAILED: &str = "Your sign-up was not saved";
const ANNOUNCE_FAILED: &str = "The update notes were not posted";
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
            clicked(
                data,
                framework.serenity_context,
                component,
                &framework.options.owners,
            )
            .await;
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
    owners: &HashSet<serenity::UserId>,
) {
    let Some(guild) = component.guild_id.map(|guild| GuildId::new(guild.get())) else {
        return;
    };
    if let Some(signup) = wiring::signup_click(&component.data.custom_id) {
        sign_up(data, serenity_context, component, signup, guild).await;
    } else if let Some(pressed) = wiring::choose_press(&component.data.custom_id) {
        choose_pressed(data, serenity_context, component, pressed, guild).await;
    } else if let Some(chosen) = wiring::hours_menu(&component.data.custom_id) {
        hours_chosen(data, serenity_context, component, chosen, guild).await;
    } else if let Some(number) = wiring::round_number(&component.data.custom_id) {
        cancel(data, serenity_context, component, number, guild).await;
    } else if let Some(click) = wiring::series_click(&component.data.custom_id) {
        series_pressed(data, serenity_context, component, click, guild).await;
    } else if let Some(version) = wiring::announce_version(&component.data.custom_id) {
        announce_pressed(data, serenity_context, component, &version, guild, owners).await;
    } else if let Some((panel, part)) = wiring::panel_part(&component.data.custom_id) {
        panel::pressed(data, serenity_context, component, panel, part, guild).await;
    }
}

async fn announce_pressed(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    version: &str,
    guild: GuildId,
    owners: &HashSet<serenity::UserId>,
) {
    let scope = Scope::of_component(component).guild(guild);
    if !acknowledge(serenity_context, component, &scope).await {
        return;
    }
    let content = if owners.contains(&component.user.id) {
        announced(data.updates.send(guild, version).await, &scope)
    } else {
        failure::refused(
            "interaction.refused",
            "a Send click came from someone who does not own Barnacle",
            "not_an_owner",
            &scope,
        );
        text::ANNOUNCE_OWNERS_ONLY.to_owned()
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

fn announced(outcome: SendOutcome, scope: &Scope) -> String {
    match outcome {
        SendOutcome::Sent { channel } => {
            failure::record(
                "updates.announced",
                "Barnacle's update notes were posted",
                &scope.clone().channel(channel),
            );
            text::announce_posted(channel)
        }
        SendOutcome::NotSetUp => text::UPDATES_NOT_SET_UP.to_owned(),
        SendOutcome::AlreadySent { version } => text::updates_already_sent(&version),
        SendOutcome::VersionChanged { running } => text::announce_version_changed(&running),
        SendOutcome::PostFailed(error) => announce_failed(
            "Barnacle's update notes could not be posted",
            &Failure::from_error(&error),
            scope,
        ),
        SendOutcome::Storage(error) => announce_failed(
            "the record of sent update notes could not be read or saved",
            &Failure::from_error(&error),
            scope,
        ),
    }
}

fn announce_failed(summary: &str, failure: &Failure, scope: &Scope) -> String {
    failure::report("interaction.failed", summary, failure, scope);
    failure::failed(ANNOUNCE_FAILED, failure)
}

struct Press {
    scope: Scope,
    place: Place,
    user: UserId,
    can_manage_messages: bool,
}

async fn take_press(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    number: u64,
    guild: GuildId,
) -> Option<Press> {
    let press = Press {
        scope: Scope {
            round: u32::try_from(number).ok(),
            ..Scope::of_component(component).guild(guild)
        },
        place: Place {
            guild,
            channel: ChannelId::new(component.channel_id.get()),
        },
        user: UserId::new(component.user.id.get()),
        can_manage_messages: component
            .member
            .as_ref()
            .and_then(|member| member.permissions)
            .is_some_and(|permissions| permissions.manage_messages()),
    };
    acknowledge(serenity_context, component, &press.scope)
        .await
        .then_some(press)
}

async fn answer_press(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    outcome: CancelOutcome,
    refused: (&'static str, &'static str),
    summary: &str,
    scope: &Scope,
) {
    let (reason, content) = match outcome {
        CancelOutcome::Cancelled => return,
        CancelOutcome::Refused => refused,
        CancelOutcome::AlreadyOver => ("round_over", text::ROUND_OVER),
    };
    failure::refused("interaction.refused", summary, reason, scope);
    tell(
        serenity_context,
        component,
        Answer::Followup,
        content,
        scope,
    )
    .await;
}

async fn cancel(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    number: u64,
    guild: GuildId,
) {
    let Some(press) = take_press(serenity_context, component, number, guild).await else {
        return;
    };
    let outcome = data
        .table
        .cancel(press.place, number, press.user, press.can_manage_messages)
        .await;
    answer_press(
        serenity_context,
        component,
        outcome,
        ("not_your_round", text::CANCEL_REFUSED),
        "a cancel click was turned down",
        &press.scope,
    )
    .await;
}

async fn series_pressed(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    click: SeriesClick,
    guild: GuildId,
) {
    let (SeriesClick::Skip(number) | SeriesClick::End(number) | SeriesClick::StillPlaying(number)) =
        click;
    let Some(press) = take_press(serenity_context, component, number, guild).await else {
        return;
    };
    let outcome = match click {
        SeriesClick::Skip(number) => {
            data.table
                .skip(press.place, number, press.user, press.can_manage_messages)
                .await
        }
        SeriesClick::End(number) => {
            data.table
                .end_series(press.place, number, press.user, press.can_manage_messages)
                .await
        }
        SeriesClick::StillPlaying(series) => data.table.still_playing(press.place, series).await,
    };
    answer_press(
        serenity_context,
        component,
        outcome,
        ("not_your_series", text::SERIES_REFUSED),
        "a series click was turned down",
        &press.scope,
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
    let scope = session_scope(
        component,
        guild,
        SessionRef {
            event: signup.event,
            date: signup.date,
        },
    );
    let now_unix = super::now_unix();
    if !acknowledge(serenity_context, component, &scope).await {
        return;
    }
    let now_ms = match signup_now_ms(&scope) {
        Ok(now_ms) => now_ms,
        Err(content) => {
            tell(
                serenity_context,
                component,
                Answer::Followup,
                &content,
                &scope,
            )
            .await;
            return;
        }
    };
    let click = Click {
        guild,
        channel: ChannelId::new(component.channel_id.get()),
        message: Snowflake::new(component.message.id.get()),
        user: UserId::new(component.user.id.get()),
        event: signup.event,
        date: signup.date,
        target: signup.target,
        attending: signup.attending,
    };
    let content = match data.signups.click(click, now_unix, now_ms).await {
        ClickOutcome::Recorded => return,
        ClickOutcome::Closed { start_unix } => {
            signups_closed(start_unix, "a sign-up click", &scope)
        }
        ClickOutcome::UnknownEvent => session_gone("a sign-up click", &scope),
        ClickOutcome::Failed(failure) => {
            signup_failed(&failure, "a sign-up could not be saved", &scope)
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

fn signup_now_ms(scope: &Scope) -> Result<u64, String> {
    super::now_ms().ok_or_else(|| {
        let failure = Failure::internal("the clock reads before 1970");
        failure::report(
            "interaction.failed",
            "a sign-up could not be saved because the clock reads before 1970",
            &failure,
            scope,
        );
        failure::failed(SIGNUP_FAILED, &failure)
    })
}

fn session_scope(
    component: &serenity::ComponentInteraction,
    guild: GuildId,
    at: SessionRef,
) -> Scope {
    Scope::of_component(component)
        .guild(guild)
        .season(at.event)
        .night(at.date.to_string())
}

fn signups_closed(start_unix: i64, what: &str, scope: &Scope) -> String {
    failure::refused(
        "interaction.refused",
        &format!("{what} came after sign-ups closed"),
        "signups_closed",
        scope,
    );
    text::signups_closed_at(start_unix)
}

fn session_gone(what: &str, scope: &Scope) -> String {
    failure::refused(
        "interaction.refused",
        &format!("{what} named a session that is gone"),
        "session_gone",
        scope,
    );
    text::SESSION_GONE.to_owned()
}

fn signup_failed(failure: &Failure, what: &str, scope: &Scope) -> String {
    failure::report("interaction.failed", what, failure, scope);
    failure::failed(SIGNUP_FAILED, failure)
}

async fn choose_pressed(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    pressed: SessionRef,
    guild: GuildId,
) {
    let scope = session_scope(component, guild, pressed);
    let now_unix = super::now_unix();
    let content = match signup_now_ms(&scope) {
        Err(content) => content,
        Ok(now_ms) => match data
            .signups
            .hours_menu(
                guild,
                pressed.event,
                pressed.date,
                UserId::new(component.user.id.get()),
                now_unix,
                now_ms,
            )
            .await
        {
            MenuOutcome::Open { view, attending } => {
                let reply = serenity::CreateInteractionResponse::Message(board::hours_menu_reply(
                    &view, &attending,
                ));
                if let Err(error) = component.create_response(serenity_context, reply).await {
                    failure::report(
                        "interaction.failed",
                        "the hours menu could not be sent",
                        &Failure::from_error(&error),
                        &scope,
                    );
                }
                return;
            }
            MenuOutcome::Closed { start_unix } => {
                signups_closed(start_unix, "a Choose hours press", &scope)
            }
            MenuOutcome::UnknownEvent => session_gone("a Choose hours press", &scope),
            MenuOutcome::Failed(failure) => {
                signup_failed(&failure, "the hours menu could not be opened", &scope)
            }
        },
    };
    tell(
        serenity_context,
        component,
        Answer::Response,
        &content,
        &scope,
    )
    .await;
}

async fn hours_chosen(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    chosen: SessionRef,
    guild: GuildId,
) {
    let scope = session_scope(component, guild, chosen);
    let now_unix = super::now_unix();
    if !acknowledge(serenity_context, component, &scope).await {
        return;
    }
    let values = match &component.data.kind {
        serenity::ComponentInteractionDataKind::StringSelect { values } => Some(values.clone()),
        _ => None,
    };
    let content = match (signup_now_ms(&scope), values) {
        (Err(content), _) => content,
        (Ok(_), None) => hours_refused(&scope),
        (Ok(now_ms), Some(values)) => {
            let pick = HoursPick {
                guild,
                channel: ChannelId::new(component.channel_id.get()),
                user: UserId::new(component.user.id.get()),
                event: chosen.event,
                date: chosen.date,
                values,
            };
            match data.signups.choose_hours(pick, now_unix, now_ms).await {
                ChooseOutcome::Saved { attending } => text::hours_saved(&attending),
                ChooseOutcome::Closed { start_unix } => {
                    signups_closed(start_unix, "an hours choice", &scope)
                }
                ChooseOutcome::UnknownEvent => session_gone("an hours choice", &scope),
                ChooseOutcome::BadChoice => hours_refused(&scope),
                ChooseOutcome::Failed(failure) => {
                    signup_failed(&failure, "an hours choice could not be saved", &scope)
                }
            }
        }
    };
    let edited = component
        .edit_response(
            serenity_context,
            serenity::EditInteractionResponse::new()
                .content(content)
                .components(Vec::new()),
        )
        .await;
    if let Err(error) = edited {
        failure::report(
            "interaction.failed",
            "the answer to an hours choice could not be sent",
            &Failure::from_error(&error),
            &scope,
        );
    }
}

fn hours_refused(scope: &Scope) -> String {
    failure::refused(
        "interaction.refused",
        "an hours choice named hours that are not part of the session",
        "hours_refused",
        scope,
    );
    text::HOURS_REFUSED.to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Answer {
    Response,
    Followup,
}

pub(super) async fn acknowledge(
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

pub(super) async fn tell(
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
                            .allowed_mentions(serenity::CreateAllowedMentions::new())
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
                    .allowed_mentions(serenity::CreateAllowedMentions::new())
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
            let command = &ctx.command().qualified_name;
            let content = match OptionProblem::of(&*error) {
                OptionProblem::Failed(failure) => {
                    failure::report(
                        "command.failed",
                        "a command option could not be looked up",
                        &failure,
                        &scope,
                    );
                    failure::command_failed(command, &failure)
                }
                OptionProblem::Unreadable(detail) => {
                    failure::refused_because(
                        "command.refused",
                        "a command option could not be read",
                        "invalid_option",
                        &detail,
                        &scope,
                    );
                    format!(
                        "Barnacle could not read one of the options you gave `/{command}`: {error}. Check it and try again."
                    )
                }
            };
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

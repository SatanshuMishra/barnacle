use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use super::Data;
use super::commands;
use super::events::Answer;
use super::events::acknowledge;
use super::events::tell;
use crate::attendance::EditReport;
use crate::event_draft::Draft;
use crate::event_draft::Purpose;
use crate::event_draft::Ready;
use crate::failure;
use crate::failure::Failure;
use crate::failure::Scope;
use crate::ids::GuildId;
use crate::ids::Ping;
use crate::ids::Place;
use crate::ids::RoleId;
use crate::schedule;
use crate::text;
use crate::text::Refusal;
use crate::wiring::PanelPart;
use crate::wiring::PingVerdict;

const PANEL_FAILED: &str = "The event was not saved";
const CLOCK_BEFORE_1970: &str = "the clock reads before 1970";
const CLOCK_OUT_OF_RANGE: &str = "the clock reads a time outside the supported calendar";

pub(super) async fn pressed(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    panel: u64,
    part: PanelPart,
    guild: GuildId,
) {
    let scope = Scope::of_component(component).guild(guild);
    let user = UserId::new(component.user.id.get());
    let Some(now_ms) = super::now_ms() else {
        let content = save_failed(
            "an event panel could not be used because the clock reads before 1970",
            &Failure::internal(CLOCK_BEFORE_1970),
            &scope,
        );
        tell(
            serenity_context,
            component,
            Answer::Response,
            &content,
            &scope,
        )
        .await;
        return;
    };
    match part {
        PanelPart::Save => {
            save(
                data,
                serenity_context,
                component,
                panel,
                user,
                now_ms,
                &scope,
            )
            .await
        }
        PanelPart::Cancel => match data.drafts.take(panel, user, now_ms) {
            Ok(_) => {
                update(
                    serenity_context,
                    component,
                    text::PANEL_CANCELLED.to_owned(),
                    Vec::new(),
                    &scope,
                )
                .await;
            }
            Err(refusal) => {
                let content = refusal_text(&refusal, &scope);
                tell(
                    serenity_context,
                    component,
                    Answer::Response,
                    &content,
                    &scope,
                )
                .await;
            }
        },
        PanelPart::Days | PanelPart::Roles | PanelPart::Everyone => {
            let redrawn = data
                .drafts
                .with(panel, user, now_ms)
                .and_then(|draft| changed(&draft, part, &component.data.kind));
            match redrawn {
                Ok(draft) => {
                    let view = draft.panel(panel, super::now_unix());
                    data.drafts.replace(panel, draft);
                    update(
                        serenity_context,
                        component,
                        view.content,
                        view.components,
                        &scope,
                    )
                    .await;
                }
                Err(refusal) => {
                    let content = refusal_text(&refusal, &scope);
                    tell(
                        serenity_context,
                        component,
                        Answer::Response,
                        &content,
                        &scope,
                    )
                    .await;
                }
            }
        }
    }
}

fn changed(
    draft: &Draft,
    part: PanelPart,
    kind: &serenity::ComponentInteractionDataKind,
) -> Result<Draft, Refusal> {
    match (part, kind) {
        (PanelPart::Days, serenity::ComponentInteractionDataKind::StringSelect { values }) => {
            draft.with_weekdays(values)
        }
        (PanelPart::Roles, serenity::ComponentInteractionDataKind::RoleSelect { values }) => {
            let roles: Vec<RoleId> = values.iter().map(|role| RoleId::new(role.get())).collect();
            draft.with_roles(&roles)
        }
        (PanelPart::Everyone, serenity::ComponentInteractionDataKind::Button) => {
            draft.with_everyone()
        }
        _ => Err(Refusal::PanelUnknown),
    }
}

async fn save(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    panel: u64,
    user: UserId,
    now_ms: u64,
    scope: &Scope,
) {
    if !acknowledge(serenity_context, component, scope).await {
        return;
    }
    let answer = match data.drafts.with(panel, user, now_ms) {
        Ok(draft) => saved(data, serenity_context, component, &draft, scope).await,
        Err(refusal) => Err(refusal_text(&refusal, scope)),
    };
    let content = match answer {
        Ok(content) => content,
        Err(content) => {
            tell(
                serenity_context,
                component,
                Answer::Followup,
                &content,
                scope,
            )
            .await;
            return;
        }
    };
    let _ = data.drafts.take(panel, user, now_ms);
    let edited = component
        .edit_response(
            serenity_context,
            serenity::EditInteractionResponse::new()
                .content(content)
                .components(Vec::new())
                .allowed_mentions(serenity::CreateAllowedMentions::new()),
        )
        .await;
    if let Err(error) = edited {
        failure::report(
            "interaction.failed",
            "the answer to an event panel could not be sent",
            &Failure::from_error(&error),
            scope,
        );
    }
}

async fn saved(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    draft: &Draft,
    scope: &Scope,
) -> Result<String, String> {
    let now_unix = super::now_unix();
    let creating = draft.purpose() == Purpose::Create;
    let place = Place {
        guild: draft.guild(),
        channel: draft.channel(),
    };
    let granted = if draft.channel().get() == component.channel_id.get() {
        component.app_permissions
    } else {
        failure::barnacle_permissions_in(&serenity_context.cache, draft.guild(), draft.channel())
    };
    let checked = commands::judge_pings(
        &serenity_context.cache,
        &draft.pings(),
        place,
        granted,
        creating,
    );
    let verdicts: Vec<(Ping, PingVerdict)> = checked
        .iter()
        .map(|checked| (checked.ping, checked.verdict))
        .collect();
    let answer = match draft.ready(now_unix, &verdicts) {
        Err(refusal) => Err(refusal_text(&refusal, scope)),
        Ok(Ready::Create { new, warning }) => match data.signups.store().create_event(&new).await {
            Ok(outcome) => draft
                .saved_reply(&outcome, warning, now_unix)
                .map_err(|refusal| refusal_text(&refusal, scope)),
            Err(error) => Err(save_failed(
                "an event could not be saved",
                &Failure::from_error(&error),
                scope,
            )),
        },
        Ok(Ready::Edit {
            event,
            created_at_ms,
            change,
            warning,
        }) => {
            let report = match schedule::today(now_unix) {
                Some(today) => {
                    data.signups
                        .edit_event(
                            draft.guild(),
                            event,
                            created_at_ms,
                            &change,
                            today,
                            now_unix,
                        )
                        .await
                }
                None => EditReport::Failed(Failure::internal(CLOCK_OUT_OF_RANGE)),
            };
            edit_answer(report, draft, warning, scope)
        }
    };
    commands::log_ping_checks(&checked, answer.is_err(), scope);
    answer
}

fn edit_answer(
    report: EditReport,
    draft: &Draft,
    warning: Option<String>,
    scope: &Scope,
) -> Result<String, String> {
    match report {
        EditReport::Edited {
            after,
            cleared,
            refreshed,
            ..
        } => Ok(text::warned(
            warning,
            text::event_edited(&after, &cleared, &refreshed),
        )),
        EditReport::ClearFailed { failures } => {
            Err(text::failures_noted(text::CLEAR_FAILED_MOVE, failures))
        }
        EditReport::NameTaken => Err(refusal_text(
            &Refusal::NameTaken(draft.name().to_owned()),
            scope,
        )),
        EditReport::BadRange => Err(refusal_text(&Refusal::LastDayBeforeFirst, scope)),
        EditReport::TooFar => Err(refusal_text(&Refusal::DatesTooFar, scope)),
        EditReport::NotFound => Err(refusal_text(&Refusal::EventNotFound, scope)),
        EditReport::Failed(failure) => Err(save_failed(
            "an event edit could not be saved",
            &failure,
            scope,
        )),
    }
}

fn refusal_text(refusal: &Refusal, scope: &Scope) -> String {
    failure::refused(
        "interaction.refused",
        "an event panel press was refused",
        refusal.code(),
        scope,
    );
    refusal.message()
}

fn save_failed(summary: &str, failure: &Failure, scope: &Scope) -> String {
    failure::report("interaction.failed", summary, failure, scope);
    failure::failed(PANEL_FAILED, failure)
}

async fn update(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    content: String,
    components: Vec<serenity::CreateActionRow>,
    scope: &Scope,
) {
    let updated = component
        .create_response(
            serenity_context,
            serenity::CreateInteractionResponse::UpdateMessage(
                serenity::CreateInteractionResponseMessage::new()
                    .content(content)
                    .components(components)
                    .allowed_mentions(serenity::CreateAllowedMentions::new()),
            ),
        )
        .await;
    if let Err(error) = updated {
        failure::report(
            "interaction.failed",
            "an event panel could not be redrawn",
            &Failure::from_error(&error),
            scope,
        );
    }
}

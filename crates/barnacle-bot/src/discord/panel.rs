use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use super::Data;
use super::commands;
use super::commands::CheckedPing;
use super::events::Answer;
use super::events::acknowledge;
use super::events::tell;
use crate::event_draft;
use crate::event_draft::Draft;
use crate::event_draft::Drafts;
use crate::event_draft::PanelView;
use crate::event_draft::Purpose;
use crate::event_draft::SaveAnswer;
use crate::failure;
use crate::failure::Failure;
use crate::failure::Scope;
use crate::ids::GuildId;
use crate::ids::Place;
use crate::ids::RoleId;
use crate::text::Refusal;
use crate::wiring::PanelPart;

const PANEL_FAILED: &str = "The event was not saved";
const CLOCK_BEFORE_1970: &str = "the clock reads before 1970";

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
        PanelPart::Cancel => match event_draft::cancel_pressed(&data.drafts, panel, user, now_ms) {
            Ok(content) => {
                update(serenity_context, component, content, Vec::new(), &scope).await;
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
            match changed(
                &data.drafts,
                panel,
                part,
                &component.data.kind,
                user,
                now_ms,
            ) {
                Ok(view) => {
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
    drafts: &Drafts,
    panel: u64,
    part: PanelPart,
    kind: &serenity::ComponentInteractionDataKind,
    user: UserId,
    now_ms: u64,
) -> Result<PanelView, Refusal> {
    match (part, kind) {
        (PanelPart::Days, serenity::ComponentInteractionDataKind::StringSelect { values }) => {
            event_draft::weekdays_picked(drafts, panel, user, values, now_ms)
        }
        (PanelPart::Roles, serenity::ComponentInteractionDataKind::RoleSelect { values }) => {
            let roles: Vec<RoleId> = values.iter().map(|role| RoleId::new(role.get())).collect();
            event_draft::roles_picked(drafts, panel, user, &roles, now_ms)
        }
        (PanelPart::Everyone, serenity::ComponentInteractionDataKind::Button) => {
            event_draft::everyone_pressed(drafts, panel, user, now_ms)
        }
        _ => drafts
            .with(panel, user, now_ms)
            .and(Err(Refusal::PanelUnknown)),
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
    let mut checked: Vec<CheckedPing> = Vec::new();
    let answer = event_draft::save_pressed(
        &data.drafts,
        panel,
        user,
        &data.signups,
        |draft| {
            checked = judged(serenity_context, component, draft);
            checked
                .iter()
                .map(|checked| (checked.ping, checked.verdict))
                .collect()
        },
        now_ms,
    )
    .await;
    let answer = match answer {
        SaveAnswer::Saved(content) => Ok(content),
        SaveAnswer::Unsaved(content) => Err(content),
        SaveAnswer::Refused(refusal) => Err(refusal_text(&refusal, scope)),
        SaveAnswer::Failed { summary, failure } => Err(save_failed(summary, &failure, scope)),
    };
    commands::log_ping_checks(&checked, answer.is_err(), scope);
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

fn judged(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    draft: &Draft,
) -> Vec<CheckedPing> {
    let place = Place {
        guild: draft.guild(),
        channel: draft.channel(),
    };
    let granted = if draft.channel().get() == component.channel_id.get() {
        component.app_permissions
    } else {
        failure::barnacle_permissions_in(&serenity_context.cache, draft.guild(), draft.channel())
    };
    commands::judge_pings(
        &serenity_context.cache,
        &draft.pings(),
        place,
        granted,
        draft.purpose() == Purpose::Create,
    )
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

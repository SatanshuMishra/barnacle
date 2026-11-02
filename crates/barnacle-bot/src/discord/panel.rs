use barnacle_guess::Snowflake;
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
use crate::event_draft::PanelView;
use crate::event_draft::Purpose;
use crate::event_draft::SaveAnswer;
use crate::failure;
use crate::failure::Failure;
use crate::failure::Scope;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Place;
use crate::ids::RoleId;
use crate::panels::Drafts;
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
            Err(refusal) => refuse(serenity_context, component, &refusal, &scope).await,
        },
        PanelPart::TypeDates => {
            let opened = event_draft::modal_opened(
                &data.drafts,
                panel,
                user,
                now_ms,
                event_draft::dates_modal,
            );
            open_modal(serenity_context, component, opened, &scope).await;
        }
        PanelPart::Zone if picked(&component.data.kind) == Some(event_draft::ANOTHER_ZONE) => {
            let opened = event_draft::modal_opened(
                &data.drafts,
                panel,
                user,
                now_ms,
                event_draft::zone_modal,
            );
            open_modal(serenity_context, component, opened, &scope).await;
        }
        PanelPart::Days
        | PanelPart::Roles
        | PanelPart::Everyone
        | PanelPart::FirstDay
        | PanelPart::LastDay
        | PanelPart::Hour
        | PanelPart::Minute
        | PanelPart::Length
        | PanelPart::Zone
        | PanelPart::Next
        | PanelPart::Back
        | PanelPart::DatesModal
        | PanelPart::ZoneModal => {
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
                Err(refusal) => refuse(serenity_context, component, &refusal, &scope).await,
            }
        }
    }
}

pub(super) async fn submitted(
    data: &Data,
    serenity_context: &serenity::Context,
    modal: &serenity::ModalInteraction,
    panel: u64,
    part: PanelPart,
    guild: GuildId,
) {
    let scope = Scope {
        guild: Some(guild),
        channel: Some(ChannelId::new(modal.channel_id.get())),
        user: Some(UserId::new(modal.user.id.get())),
        interaction: Some(modal.id.get()),
        message: modal
            .message
            .as_ref()
            .map(|message| Snowflake::new(message.id.get())),
        ..Scope::default()
    };
    let user = UserId::new(modal.user.id.get());
    let Some(now_ms) = super::now_ms() else {
        let content = save_failed(
            "an event panel could not be used because the clock reads before 1970",
            &Failure::internal(CLOCK_BEFORE_1970),
            &scope,
        );
        answer_modal(serenity_context, modal, Err(content), &scope).await;
        return;
    };
    let typed = |id: &str| typed_value(&modal.data, id);
    let redrawn = match part {
        PanelPart::DatesModal => event_draft::dates_typed(
            &data.drafts,
            panel,
            user,
            &typed(event_draft::FIRST_DAY_INPUT),
            &typed(event_draft::LAST_DAY_INPUT),
            now_ms,
        ),
        PanelPart::ZoneModal => {
            let zone = typed(event_draft::ZONE_INPUT);
            event_draft::redrawn(&data.drafts, panel, user, now_ms, |draft| {
                draft.with_zone(&zone)
            })
        }
        PanelPart::Days
        | PanelPart::Roles
        | PanelPart::Everyone
        | PanelPart::Save
        | PanelPart::Cancel
        | PanelPart::FirstDay
        | PanelPart::LastDay
        | PanelPart::TypeDates
        | PanelPart::Hour
        | PanelPart::Minute
        | PanelPart::Length
        | PanelPart::Zone
        | PanelPart::Next
        | PanelPart::Back => data
            .drafts
            .with(panel, user, now_ms)
            .and(Err(Refusal::PanelUnknown)),
    };
    let answer = redrawn.map_err(|refusal| refusal_text(&refusal, &scope));
    answer_modal(serenity_context, modal, answer, &scope).await;
}

fn typed_value(data: &serenity::ModalInteractionData, id: &str) -> String {
    data.components
        .iter()
        .flat_map(|row| row.components.iter())
        .find_map(|component| match component {
            serenity::ActionRowComponent::InputText(input) if input.custom_id == id => {
                Some(input.value.clone().unwrap_or_default())
            }
            _ => None,
        })
        .unwrap_or_default()
}

fn picked(kind: &serenity::ComponentInteractionDataKind) -> Option<&str> {
    match kind {
        serenity::ComponentInteractionDataKind::StringSelect { values } => {
            values.first().map(String::as_str)
        }
        _ => None,
    }
}

fn changed(
    drafts: &Drafts<Draft>,
    panel: u64,
    part: PanelPart,
    kind: &serenity::ComponentInteractionDataKind,
    user: UserId,
    now_ms: u64,
) -> Result<PanelView, Refusal> {
    let single = picked(kind).unwrap_or_default().to_owned();
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
        (PanelPart::Next, serenity::ComponentInteractionDataKind::Button) => {
            event_draft::redrawn(drafts, panel, user, now_ms, Draft::next)
        }
        (PanelPart::Back, serenity::ComponentInteractionDataKind::Button) => {
            event_draft::redrawn(drafts, panel, user, now_ms, |draft| Ok(draft.back()))
        }
        (
            PanelPart::FirstDay
            | PanelPart::LastDay
            | PanelPart::Hour
            | PanelPart::Minute
            | PanelPart::Length
            | PanelPart::Zone,
            serenity::ComponentInteractionDataKind::StringSelect { .. },
        ) => event_draft::redrawn(drafts, panel, user, now_ms, |draft| match part {
            PanelPart::FirstDay => draft.with_first_day(&single),
            PanelPart::LastDay => draft.with_last_day(&single),
            PanelPart::Hour => draft.with_hour(&single),
            PanelPart::Minute => draft.with_minute(&single),
            PanelPart::Length => draft.with_hours(&single),
            PanelPart::Zone => draft.with_zone(&single),
            PanelPart::Days
            | PanelPart::Roles
            | PanelPart::Everyone
            | PanelPart::Save
            | PanelPart::Cancel
            | PanelPart::TypeDates
            | PanelPart::Next
            | PanelPart::Back
            | PanelPart::DatesModal
            | PanelPart::ZoneModal => Err(Refusal::PanelUnknown),
        }),
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

async fn refuse(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    refusal: &Refusal,
    scope: &Scope,
) {
    let content = refusal_text(refusal, scope);
    tell(
        serenity_context,
        component,
        Answer::Response,
        &content,
        scope,
    )
    .await;
}

async fn open_modal(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    opened: Result<serenity::CreateModal, Refusal>,
    scope: &Scope,
) {
    let modal = match opened {
        Ok(modal) => modal,
        Err(refusal) => return refuse(serenity_context, component, &refusal, scope).await,
    };
    let shown = component
        .create_response(
            serenity_context,
            serenity::CreateInteractionResponse::Modal(modal),
        )
        .await;
    if let Err(error) = shown {
        failure::report(
            "interaction.failed",
            "an event panel could not open its form",
            &Failure::from_error(&error),
            scope,
        );
    }
}

async fn answer_modal(
    serenity_context: &serenity::Context,
    modal: &serenity::ModalInteraction,
    answer: Result<PanelView, String>,
    scope: &Scope,
) {
    let response = match answer {
        Ok(view) => serenity::CreateInteractionResponse::UpdateMessage(
            serenity::CreateInteractionResponseMessage::new()
                .content(view.content)
                .components(view.components)
                .allowed_mentions(serenity::CreateAllowedMentions::new()),
        ),
        Err(content) => serenity::CreateInteractionResponse::Message(
            serenity::CreateInteractionResponseMessage::new()
                .content(content)
                .allowed_mentions(serenity::CreateAllowedMentions::new())
                .ephemeral(true),
        ),
    };
    if let Err(error) = modal.create_response(serenity_context, response).await {
        failure::report(
            "interaction.failed",
            "the answer to an event panel form could not be sent",
            &Failure::from_error(&error),
            scope,
        );
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

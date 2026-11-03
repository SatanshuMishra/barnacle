use std::collections::BTreeSet;
use std::num::NonZeroU64;

use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use super::Context;
use super::Data;
use super::Error;
use super::commands;
use super::events::Answer;
use super::events::acknowledge;
use super::events::tell;
use crate::access;
use crate::access_store::AccessStoreError;
use crate::failure;
use crate::failure::Failure;
use crate::failure::Scope;
use crate::ids::GuildId;
use crate::ids::RoleId;
use crate::panels::Owned;
use crate::text;
use crate::text::Refusal;
use crate::wiring::AccessPanelPart;
use crate::wiring::access_panel_id;

const MENU_MAX: u8 = 25;
const CLOCK_BEFORE_1970: &str = "the clock reads before 1970";
const ACCESS_PROMPT: &str = "Pick the roles whose members may run organiser commands, then press Save. Administrators can always run them.";
const ROLES_PLACEHOLDER: &str = "Organiser roles";
const SAVE_LABEL: &str = "Save";
const CANCEL_LABEL: &str = "Cancel";
const CLOSED: &str = "Closed. Organiser roles were not changed.";
const NO_ORGANISER_ROLES: &str =
    "No organiser roles set. Only Administrators can run organiser commands.";
const ADMINISTRATOR_NEEDED: &str = "Only Administrators can change organiser roles.";
const ACCESS_NOT_CHECKED: &str = "Barnacle could not check whether you are an organiser";
const ACCESS_NOT_SAVED: &str = "The organiser roles were not saved";

#[derive(Debug, Clone)]
pub struct AccessDraft {
    owner: UserId,
    roles: Vec<RoleId>,
}

impl Owned for AccessDraft {
    fn owner(&self) -> UserId {
        self.owner
    }
}

pub(super) fn commands() -> Vec<poise::Command<Data, Error>> {
    vec![poise::Command {
        description: Some("Barnacle settings for this server".into()),
        subcommands: vec![poise::Command {
            description: Some("Choose which roles may run organiser commands".into()),
            ..barnacle_access()
        }],
        subcommand_required: true,
        ..barnacle()
    }]
}

#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
async fn barnacle(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, rename = "access")]
async fn barnacle_access(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = ctx.guild_id().map(|guild| GuildId::new(guild.get())) else {
        return Ok(());
    };
    let Some(now_ms) = super::now_ms() else {
        return commands::fail(ctx, Failure::internal(CLOCK_BEFORE_1970)).await;
    };
    let roles = ctx.data().access.organiser_roles(guild).await?;
    let panel = ctx.id();
    let reply = commands::private_reply(ACCESS_PROMPT).components(access_rows(panel, &roles));
    ctx.data().access_drafts.open(
        panel,
        AccessDraft {
            owner: UserId::new(ctx.author().id.get()),
            roles,
        },
        now_ms,
    );
    ctx.send(reply).await?;
    Ok(())
}

pub fn access_rows(panel: u64, roles: &[RoleId]) -> Vec<serenity::CreateActionRow> {
    let defaults: Vec<serenity::RoleId> = roles
        .iter()
        .filter_map(|role| NonZeroU64::new(role.get()).map(serenity::RoleId::from))
        .take(usize::from(MENU_MAX))
        .collect();
    vec![
        serenity::CreateActionRow::SelectMenu(
            serenity::CreateSelectMenu::new(
                access_panel_id(panel, AccessPanelPart::Roles),
                serenity::CreateSelectMenuKind::Role {
                    default_roles: (!defaults.is_empty()).then_some(defaults),
                },
            )
            .placeholder(ROLES_PLACEHOLDER)
            .min_values(0)
            .max_values(MENU_MAX),
        ),
        serenity::CreateActionRow::Buttons(vec![
            serenity::CreateButton::new(access_panel_id(panel, AccessPanelPart::Save))
                .label(SAVE_LABEL)
                .style(serenity::ButtonStyle::Primary),
            serenity::CreateButton::new(access_panel_id(panel, AccessPanelPart::Cancel))
                .label(CANCEL_LABEL)
                .style(serenity::ButtonStyle::Secondary),
        ]),
    ]
}

pub fn organiser_roles_text(roles: &[RoleId]) -> String {
    if roles.is_empty() {
        return NO_ORGANISER_ROLES.to_owned();
    }
    let mentions: Vec<String> = roles
        .iter()
        .map(|role| format!("<@&{}>", role.get()))
        .collect();
    format!("Organiser roles: {}.", mentions.join(", "))
}

pub(super) async fn organiser_check(ctx: Context<'_>) -> Result<bool, Error> {
    let scope = Scope::of_command(ctx);
    let Some(guild) = ctx.guild_id().map(|guild| GuildId::new(guild.get())) else {
        failure::refused(
            "command.refused",
            "an organiser command was used outside a server",
            "guild_only",
            &scope,
        );
        return Ok(false);
    };
    let allowed = match ctx.author_member().await {
        Some(member) => may_organise_in(ctx.data(), guild, &member).await?,
        None => false,
    };
    if allowed || autocompleting(ctx) {
        return Ok(allowed);
    }
    failure::refused(
        "command.refused",
        "a member who is not an organiser used an organiser command",
        "not_an_organiser",
        &scope,
    );
    ctx.send(commands::private_reply(text::NOT_AN_ORGANISER))
        .await?;
    Ok(false)
}

pub(super) async fn organiser_pressed(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    guild: GuildId,
    scope: &Scope,
) -> bool {
    let allowed = match component.member.as_ref() {
        Some(member) => may_organise_in(data, guild, member).await,
        None => Ok(false),
    };
    let content = match allowed {
        Ok(true) => return true,
        Ok(false) => {
            failure::refused(
                "interaction.refused",
                "a panel button was pressed by a member who is not an organiser",
                "not_an_organiser",
                scope,
            );
            text::NOT_AN_ORGANISER.to_owned()
        }
        Err(error) => {
            let failure = Failure::from_error(&error);
            failure::report(
                "interaction.failed",
                "organiser roles could not be read for a panel button",
                &failure,
                scope,
            );
            failure::failed(ACCESS_NOT_CHECKED, &failure)
        }
    };
    tell(
        serenity_context,
        component,
        Answer::Response,
        &content,
        scope,
    )
    .await;
    false
}

async fn may_organise_in(
    data: &Data,
    guild: GuildId,
    member: &serenity::Member,
) -> Result<bool, AccessStoreError> {
    let organiser_roles = data.access.organiser_roles(guild).await?;
    let member_roles: Vec<RoleId> = member
        .roles
        .iter()
        .map(|role| RoleId::new(role.get()))
        .collect();
    Ok(access::may_organise(
        is_administrator(member),
        &member_roles,
        &organiser_roles,
    ))
}

fn is_administrator(member: &serenity::Member) -> bool {
    member
        .permissions
        .is_some_and(|permissions| permissions.administrator())
}

fn autocompleting(ctx: Context<'_>) -> bool {
    matches!(
        ctx,
        poise::Context::Application(application)
            if application.interaction_type == poise::CommandInteractionType::Autocomplete
    )
}

pub(super) async fn pressed(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    panel: u64,
    part: AccessPanelPart,
    guild: GuildId,
) {
    let scope = Scope::of_component(component).guild(guild);
    let user = UserId::new(component.user.id.get());
    let Some(now_ms) = super::now_ms() else {
        let failure = Failure::internal(CLOCK_BEFORE_1970);
        failure::report(
            "interaction.failed",
            "the organiser roles panel could not be used because the clock reads before 1970",
            &failure,
            &scope,
        );
        let content = failure::failed(ACCESS_NOT_SAVED, &failure);
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
        AccessPanelPart::Roles => {
            roles_picked(
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
        AccessPanelPart::Save => {
            save(
                data,
                serenity_context,
                component,
                panel,
                user,
                now_ms,
                guild,
                &scope,
            )
            .await
        }
        AccessPanelPart::Cancel => {
            cancel(
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
    }
}

async fn roles_picked(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    panel: u64,
    user: UserId,
    now_ms: u64,
    scope: &Scope,
) {
    let draft = match data.access_drafts.with(panel, user, now_ms) {
        Ok(draft) => draft,
        Err(refusal) => return refuse(serenity_context, component, &refusal, scope).await,
    };
    let roles = match &component.data.kind {
        serenity::ComponentInteractionDataKind::RoleSelect { values } => {
            values.iter().map(|role| RoleId::new(role.get())).collect()
        }
        _ => return refuse(serenity_context, component, &Refusal::PanelUnknown, scope).await,
    };
    data.access_drafts
        .replace(panel, AccessDraft { roles, ..draft });
    acknowledge(serenity_context, component, scope).await;
}

#[allow(clippy::too_many_arguments)]
async fn save(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    panel: u64,
    user: UserId,
    now_ms: u64,
    guild: GuildId,
    scope: &Scope,
) {
    if let Err(refusal) = data.access_drafts.with(panel, user, now_ms) {
        return refuse(serenity_context, component, &refusal, scope).await;
    }
    if !component.member.as_ref().is_some_and(is_administrator) {
        failure::refused(
            "interaction.refused",
            "organiser roles were saved by a member who is not an Administrator",
            "missing_user_permissions",
            scope,
        );
        tell(
            serenity_context,
            component,
            Answer::Response,
            ADMINISTRATOR_NEEDED,
            scope,
        )
        .await;
        return;
    }
    let draft = match data.access_drafts.take(panel, user, now_ms) {
        Ok(draft) => draft,
        Err(refusal) => return refuse(serenity_context, component, &refusal, scope).await,
    };
    if !acknowledge(serenity_context, component, scope).await {
        data.access_drafts.open(panel, draft, now_ms);
        return;
    }
    let picked: Vec<RoleId> = draft
        .roles
        .iter()
        .copied()
        .collect::<BTreeSet<RoleId>>()
        .into_iter()
        .collect();
    if let Err(error) = data.access.set_organiser_roles(guild, &picked).await {
        let failure = Failure::from_error(&error);
        failure::report(
            "interaction.failed",
            "organiser roles could not be saved",
            &failure,
            scope,
        );
        data.access_drafts.open(panel, draft, now_ms);
        tell(
            serenity_context,
            component,
            Answer::Followup,
            &failure::failed(ACCESS_NOT_SAVED, &failure),
            scope,
        )
        .await;
        return;
    }
    failure::record(
        "access.organiser_roles.saved",
        "a server chose its organiser roles",
        scope,
    );
    let edited = component
        .edit_response(
            serenity_context,
            serenity::EditInteractionResponse::new()
                .content(organiser_roles_text(&picked))
                .components(Vec::new())
                .allowed_mentions(serenity::CreateAllowedMentions::new()),
        )
        .await;
    if let Err(error) = edited {
        failure::report(
            "interaction.failed",
            "the answer to the organiser roles panel could not be sent",
            &Failure::from_error(&error),
            scope,
        );
    }
}

async fn cancel(
    data: &Data,
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    panel: u64,
    user: UserId,
    now_ms: u64,
    scope: &Scope,
) {
    match data.access_drafts.take(panel, user, now_ms) {
        Ok(_) | Err(Refusal::PanelUnknown) => {}
        Err(refusal) => return refuse(serenity_context, component, &refusal, scope).await,
    }
    failure::record(
        "access.panel.cancelled",
        "the organiser roles panel was cancelled",
        scope,
    );
    let updated = component
        .create_response(
            serenity_context,
            serenity::CreateInteractionResponse::UpdateMessage(
                serenity::CreateInteractionResponseMessage::new()
                    .content(CLOSED)
                    .components(Vec::new())
                    .allowed_mentions(serenity::CreateAllowedMentions::new()),
            ),
        )
        .await;
    if let Err(error) = updated {
        failure::report(
            "interaction.failed",
            "the organiser roles panel could not be closed",
            &Failure::from_error(&error),
            scope,
        );
    }
}

async fn refuse(
    serenity_context: &serenity::Context,
    component: &serenity::ComponentInteraction,
    refusal: &Refusal,
    scope: &Scope,
) {
    failure::refused(
        "interaction.refused",
        "an organiser roles panel press was refused",
        refusal.code(),
        scope,
    );
    tell(
        serenity_context,
        component,
        Answer::Response,
        &refusal.message(),
        scope,
    )
    .await;
}

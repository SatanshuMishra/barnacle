use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;

use barnacle_guess::UserId;
use jiff::ToSpan;
use jiff::civil::Date;
use jiff::civil::Weekday;
use poise::serenity_prelude as serenity;

use crate::attendance::Board;
use crate::attendance::EditReport;
use crate::attendance::Signups;
use crate::attendance::listed_events;
use crate::attendance::next_post_at;
use crate::attendance_store::AttendanceError;
use crate::attendance_store::CreateOutcome;
use crate::attendance_store::Event;
use crate::attendance_store::EventChange;
use crate::attendance_store::NewEvent;
use crate::failure::Failure;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Ping;
use crate::ids::RoleId;
use crate::schedule;
use crate::schedule::Hours;
use crate::schedule::RANGE_MONTHS;
use crate::schedule::Schedule;
use crate::schedule::Span;
use crate::schedule::StartTime;
use crate::schedule::Weekdays;
use crate::schedule::ZONE_NAME_LIMIT;
use crate::schedule::Zone;
use crate::text;
use crate::text::Refusal;
use crate::wiring;
use crate::wiring::PanelPart;
use crate::wiring::PingChoiceProblem;
use crate::wiring::PingVerdict;
use crate::wiring::SUGGESTION_LIMIT;
use crate::wiring::SetupPing;

pub const ROLE_LIMIT: usize = ROLE_MENU_MAX as usize;
pub const PANEL_LIFETIME_MINUTES: u64 = 15;
pub const PANEL_LIFETIME_MS: u64 = PANEL_LIFETIME_MINUTES * MILLIS_PER_MINUTE;

const MILLIS_PER_SECOND: u64 = 1_000;
const CLOCK_OUT_OF_RANGE: &str = "the clock reads a time outside the supported calendar";
const MILLIS_PER_MINUTE: u64 = 60 * MILLIS_PER_SECOND;
const ROLE_MENU_MAX: u8 = 5;
const WEEKDAY_MENU_MAX: u8 = 7;
const CLAN_BATTLES: &str = "Clan Battles";
const WEEKDAY_VALUES: [(Weekday, &str); 7] = [
    (Weekday::Monday, "mon"),
    (Weekday::Tuesday, "tue"),
    (Weekday::Wednesday, "wed"),
    (Weekday::Thursday, "thu"),
    (Weekday::Friday, "fri"),
    (Weekday::Saturday, "sat"),
    (Weekday::Sunday, "sun"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    ClanBattles,
}

impl Template {
    pub fn named(text: &str) -> Option<Template> {
        wiring::same_name(text.trim(), CLAN_BATTLES).then_some(Template::ClanBattles)
    }

    pub fn name(self) -> &'static str {
        match self {
            Template::ClanBattles => CLAN_BATTLES,
        }
    }

    pub fn schedule(self) -> Schedule {
        match self {
            Template::ClanBattles => Schedule::clan_battles(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CreateOptions {
    pub name: String,
    pub first_day: String,
    pub last_day: String,
    pub template: Option<String>,
    pub start: Option<String>,
    pub hours: Option<i64>,
    pub time_zone: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditOptions {
    pub name: Option<String>,
    pub first_day: Option<String>,
    pub last_day: Option<String>,
    pub start: Option<String>,
    pub hours: Option<i64>,
    pub time_zone: Option<String>,
    pub channel: Option<ChannelId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Create,
    Edit { event: i64, created_at_ms: u64 },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Changed {
    pub name: bool,
    pub first_day: bool,
    pub last_day: bool,
    pub weekdays: bool,
    pub start: bool,
    pub zone: bool,
    pub hours: bool,
    pub channel: bool,
    pub pings: bool,
}

impl Changed {
    fn schedule(self) -> bool {
        self.first_day || self.last_day || self.weekdays || self.start || self.zone
    }

    fn any(self) -> bool {
        self.schedule() || self.name || self.hours || self.channel || self.pings
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    purpose: Purpose,
    owner: UserId,
    guild: GuildId,
    channel: ChannelId,
    name: String,
    span: Span,
    everyone: bool,
    roles: Vec<RoleId>,
    changed: Changed,
}

#[derive(Debug, Clone)]
pub struct PanelView {
    pub content: String,
    pub components: Vec<serenity::CreateActionRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ready {
    Create {
        new: NewEvent,
        warning: Option<String>,
    },
    Edit {
        event: i64,
        created_at_ms: u64,
        change: EventChange,
        warning: Option<String>,
    },
}

#[derive(Debug)]
pub enum Applied {
    Create {
        outcome: Result<CreateOutcome, AttendanceError>,
        warning: Option<String>,
    },
    Edit {
        report: EditReport,
        warning: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveAnswer {
    Saved(String),
    Unsaved(String),
    Refused(Refusal),
    Failed {
        summary: &'static str,
        failure: Failure,
    },
}

impl Ready {
    pub async fn apply<B: Board>(
        self,
        signups: &Arc<Signups<B>>,
        guild: GuildId,
        now_unix: i64,
    ) -> Applied {
        match self {
            Ready::Create { new, warning } => Applied::Create {
                outcome: signups.create_event(&new).await,
                warning,
            },
            Ready::Edit {
                event,
                created_at_ms,
                change,
                warning,
            } => {
                let report = match schedule::today(now_unix) {
                    Some(today) => {
                        signups
                            .edit_event(guild, event, created_at_ms, &change, today, now_unix)
                            .await
                    }
                    None => EditReport::Failed(Failure::internal(CLOCK_OUT_OF_RANGE)),
                };
                Applied::Edit { report, warning }
            }
        }
    }
}

impl Draft {
    pub fn create(
        options: &CreateOptions,
        guild: GuildId,
        channel: ChannelId,
        owner: UserId,
        now_unix: i64,
    ) -> Result<Draft, Refusal> {
        let name = wiring::event_name(&options.name).ok_or(Refusal::NameFormat)?;
        let template = options
            .template
            .as_deref()
            .map(|text| Template::named(text).ok_or(Refusal::TemplateUnknown))
            .transpose()?;
        let base = template.map(Template::schedule);
        let start = options
            .start
            .as_deref()
            .map(read_start)
            .transpose()?
            .or_else(|| base.as_ref().map(Schedule::start));
        let hours = options
            .hours
            .map(read_hours)
            .transpose()?
            .or_else(|| base.as_ref().map(Schedule::hours));
        let zone = options
            .time_zone
            .as_deref()
            .map(read_zone)
            .transpose()?
            .or_else(|| base.as_ref().map(|base| base.zone().clone()));
        let missing: Vec<&'static str> = [
            ("start", start.is_none()),
            ("hours", hours.is_none()),
            ("time_zone", zone.is_none()),
        ]
        .into_iter()
        .filter_map(|(option, absent)| absent.then_some(option))
        .collect();
        let (Some(start), Some(hours), Some(zone)) = (start, hours, zone) else {
            return Err(Refusal::MissingOptions(missing));
        };
        let first_day = read_day(&options.first_day)?;
        let last_day = read_day(&options.last_day)?;
        let weekdays = match &base {
            Some(base) => base.weekdays(),
            None => Weekdays::new(&[first_day.weekday()]).ok_or(Refusal::WeekdaysInvalid)?,
        };
        let span = settled_span(
            first_day,
            last_day,
            Schedule::new(weekdays, start, zone, hours),
            now_unix,
        )?;
        if span.sessions_left(now_unix) == 0 {
            return Err(Refusal::NoSessionsLeft);
        }
        Ok(Draft {
            purpose: Purpose::Create,
            owner,
            guild,
            channel,
            name,
            span,
            everyone: false,
            roles: Vec::new(),
            changed: Changed::default(),
        })
    }

    pub fn edit(
        event: &Event,
        options: &EditOptions,
        owner: UserId,
        now_unix: i64,
    ) -> Result<Draft, Refusal> {
        let held = event.span.schedule();
        let name = options
            .name
            .as_deref()
            .map(|text| wiring::event_name(text).ok_or(Refusal::NameFormat))
            .transpose()?;
        let first_day = options.first_day.as_deref().map(read_day).transpose()?;
        let last_day = options.last_day.as_deref().map(read_day).transpose()?;
        let start = options.start.as_deref().map(read_start).transpose()?;
        let hours = options.hours.map(read_hours).transpose()?;
        let zone = options.time_zone.as_deref().map(read_zone).transpose()?;
        let changed = Changed {
            name: name.is_some(),
            first_day: first_day.is_some(),
            last_day: last_day.is_some(),
            weekdays: false,
            start: start.is_some(),
            zone: zone.is_some(),
            hours: hours.is_some(),
            channel: options.channel.is_some(),
            pings: false,
        };
        let span = settled_span(
            first_day.unwrap_or(event.span.first_day()),
            last_day.unwrap_or(event.span.last_day()),
            Schedule::new(
                held.weekdays(),
                start.unwrap_or(held.start()),
                zone.unwrap_or_else(|| held.zone().clone()),
                hours.unwrap_or(held.hours()),
            ),
            now_unix,
        )?;
        if changed.schedule() && span.sessions_left(now_unix) == 0 {
            return Err(Refusal::NoSessionsLeft);
        }
        let everyone = everyone_role(event.guild);
        Ok(Draft {
            purpose: Purpose::Edit {
                event: event.id,
                created_at_ms: event.created_at_ms,
            },
            owner,
            guild: event.guild,
            channel: options.channel.unwrap_or(event.channel),
            name: name.unwrap_or_else(|| event.name.clone()),
            span,
            everyone: event.ping_roles.contains(&everyone),
            roles: event
                .ping_roles
                .iter()
                .copied()
                .filter(|role| *role != everyone)
                .collect(),
            changed,
        })
    }

    pub fn purpose(&self) -> Purpose {
        self.purpose
    }

    pub fn owner(&self) -> UserId {
        self.owner
    }

    pub fn guild(&self) -> GuildId {
        self.guild
    }

    pub fn channel(&self) -> ChannelId {
        self.channel
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn span(&self) -> &Span {
        &self.span
    }

    pub fn everyone(&self) -> bool {
        self.everyone
    }

    pub fn roles(&self) -> &[RoleId] {
        &self.roles
    }

    pub fn changed(&self) -> Changed {
        self.changed
    }

    pub fn pings(&self) -> Vec<Ping> {
        if self.everyone {
            vec![Ping::Everyone]
        } else {
            self.roles.iter().copied().map(Ping::Role).collect()
        }
    }

    pub fn ping_roles(&self) -> Vec<RoleId> {
        if self.everyone {
            vec![everyone_role(self.guild)]
        } else {
            self.roles.clone()
        }
    }

    pub fn with_weekdays(&self, values: &[String]) -> Result<Draft, Refusal> {
        let days = values
            .iter()
            .map(|value| {
                WEEKDAY_VALUES
                    .iter()
                    .find(|(_, text)| text == value)
                    .map(|(day, _)| *day)
            })
            .collect::<Option<Vec<Weekday>>>()
            .ok_or(Refusal::WeekdaysInvalid)?;
        let weekdays = Weekdays::new(&days).ok_or(Refusal::WeekdaysInvalid)?;
        let held = self.span.schedule();
        let span = Span::new(
            self.span.first_day(),
            self.span.last_day(),
            Schedule::new(weekdays, held.start(), held.zone().clone(), held.hours()),
        )
        .ok_or(Refusal::LastDayBeforeFirst)?;
        Ok(Draft {
            span,
            changed: Changed {
                weekdays: true,
                ..self.changed
            },
            ..self.clone()
        })
    }

    pub fn with_roles(&self, roles: &[RoleId]) -> Result<Draft, Refusal> {
        if roles.len() > ROLE_LIMIT {
            return Err(Refusal::TooManyRoles);
        }
        let chosen = wiring::ping_choice(
            self.guild,
            std::array::from_fn(|index| roles.get(index).copied()),
        )
        .map_err(|problem| match problem {
            PingChoiceProblem::MixedEveryone => Refusal::PingMixesEveryone,
        })?;
        let pings_changed = Changed {
            pings: true,
            ..self.changed
        };
        if chosen == [everyone_role(self.guild)] {
            return Ok(Draft {
                everyone: true,
                roles: Vec::new(),
                changed: pings_changed,
                ..self.clone()
            });
        }
        if self.everyone && !chosen.is_empty() {
            return Err(Refusal::PingMixesEveryone);
        }
        Ok(Draft {
            roles: chosen,
            changed: pings_changed,
            ..self.clone()
        })
    }

    pub fn with_everyone(&self) -> Result<Draft, Refusal> {
        if !self.everyone && !self.roles.is_empty() {
            return Err(Refusal::PingMixesEveryone);
        }
        Ok(Draft {
            everyone: !self.everyone,
            changed: Changed {
                pings: true,
                ..self.changed
            },
            ..self.clone()
        })
    }

    pub fn panel(&self, panel: u64, now_unix: i64) -> PanelView {
        let weekdays = self.span.schedule().weekdays();
        let days = serenity::CreateSelectMenu::new(
            wiring::panel_id(panel, PanelPart::Days),
            serenity::CreateSelectMenuKind::String {
                options: WEEKDAY_VALUES
                    .iter()
                    .map(|(day, value)| {
                        serenity::CreateSelectMenuOption::new(text::weekday_name(*day), *value)
                            .default_selection(weekdays.contains(*day))
                    })
                    .collect(),
            },
        )
        .placeholder(text::WEEKDAYS_PLACEHOLDER)
        .min_values(1)
        .max_values(WEEKDAY_MENU_MAX);
        let roles = serenity::CreateSelectMenu::new(
            wiring::panel_id(panel, PanelPart::Roles),
            serenity::CreateSelectMenuKind::Role {
                default_roles: (!self.roles.is_empty()).then(|| {
                    self.roles
                        .iter()
                        .filter_map(|role| NonZeroU64::new(role.get()).map(serenity::RoleId::from))
                        .collect()
                }),
            },
        )
        .placeholder(text::ROLES_PLACEHOLDER)
        .min_values(0)
        .max_values(ROLE_MENU_MAX);
        let save = match self.purpose {
            Purpose::Create => text::CREATE_LABEL,
            Purpose::Edit { .. } => text::SAVE_LABEL,
        };
        let everyone_style = if self.everyone {
            serenity::ButtonStyle::Primary
        } else {
            serenity::ButtonStyle::Secondary
        };
        let buttons = vec![
            serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::Save))
                .label(save)
                .style(serenity::ButtonStyle::Success),
            serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::Everyone))
                .label(text::everyone_label(self.everyone))
                .style(everyone_style),
            serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::Cancel))
                .label(text::CANCEL_LABEL)
                .style(serenity::ButtonStyle::Secondary),
        ];
        PanelView {
            content: text::panel_preview(
                &self.name,
                self.channel,
                &self.span,
                &self.pings(),
                now_unix,
            ),
            components: vec![
                serenity::CreateActionRow::SelectMenu(days),
                serenity::CreateActionRow::SelectMenu(roles),
                serenity::CreateActionRow::Buttons(buttons),
            ],
        }
    }

    pub fn ready(&self, now_unix: i64, verdicts: &[(Ping, PingVerdict)]) -> Result<Ready, Refusal> {
        let creating = self.purpose == Purpose::Create;
        if !creating && !self.changed.any() {
            return Err(Refusal::NothingToChange);
        }
        if !within_window(&self.span, now_unix) {
            return Err(Refusal::DatesTooFar);
        }
        if (creating || self.changed.schedule()) && self.span.sessions_left(now_unix) == 0 {
            return Err(Refusal::NoSessionsLeft);
        }
        let blocked: Vec<Ping> = verdicts
            .iter()
            .filter(|(_, verdict)| wiring::setup_ping(creating, *verdict) == SetupPing::Refuse)
            .map(|(ping, _)| *ping)
            .collect();
        if !blocked.is_empty() {
            return Err(Refusal::PingBlockedByChannel {
                pings: blocked,
                channel: self.channel,
            });
        }
        let warning = text::silent_ping_warning(verdicts, self.channel);
        Ok(match self.purpose {
            Purpose::Create => Ready::Create {
                new: NewEvent {
                    guild: self.guild,
                    channel: self.channel,
                    name: self.name.clone(),
                    span: self.span.clone(),
                    created_by: self.owner,
                    created_at_ms: u64::try_from(now_unix)
                        .unwrap_or_default()
                        .saturating_mul(MILLIS_PER_SECOND),
                    ping_roles: self.ping_roles(),
                },
                warning,
            },
            Purpose::Edit {
                event,
                created_at_ms,
            } => Ready::Edit {
                event,
                created_at_ms,
                change: self.change(),
                warning,
            },
        })
    }

    pub fn saved_reply(
        &self,
        outcome: &CreateOutcome,
        warning: Option<String>,
        now_unix: i64,
    ) -> Result<String, Refusal> {
        match outcome {
            CreateOutcome::Created(event) => Ok(text::warned(
                warning,
                text::event_created(
                    event,
                    event.span.sessions_left(now_unix),
                    next_post_at(&event.span, now_unix),
                ),
            )),
            CreateOutcome::NameTaken => Err(Refusal::NameTaken(self.name.clone())),
        }
    }

    fn change(&self) -> EventChange {
        let schedule = self.span.schedule();
        EventChange {
            name: self.changed.name.then(|| self.name.clone()),
            first_day: self.changed.first_day.then(|| self.span.first_day()),
            last_day: self.changed.last_day.then(|| self.span.last_day()),
            weekdays: self.changed.weekdays.then(|| schedule.weekdays()),
            start: self.changed.start.then(|| schedule.start()),
            zone: self.changed.zone.then(|| schedule.zone().clone()),
            hours: self.changed.hours.then(|| schedule.hours()),
            channel: self.changed.channel.then_some(self.channel),
            ping_roles: self.changed.pings.then(|| self.ping_roles()),
        }
    }
}

#[derive(Debug, Clone)]
struct Opened {
    draft: Draft,
    opened_at_ms: u64,
}

#[derive(Debug, Default)]
pub struct Drafts {
    open: Mutex<HashMap<u64, Opened>>,
}

impl Drafts {
    pub fn open(&self, panel: u64, draft: Draft, now_ms: u64) {
        let mut open = self.held();
        open.retain(|_, opened| !expired(opened, now_ms));
        open.insert(
            panel,
            Opened {
                draft,
                opened_at_ms: now_ms,
            },
        );
    }

    pub fn with(&self, panel: u64, user: UserId, now_ms: u64) -> Result<Draft, Refusal> {
        usable(self.held().get(&panel), user, now_ms).map(|opened| opened.draft.clone())
    }

    pub fn take(&self, panel: u64, user: UserId, now_ms: u64) -> Result<Draft, Refusal> {
        let mut open = self.held();
        usable(open.get(&panel), user, now_ms)?;
        open.remove(&panel)
            .map(|opened| opened.draft)
            .ok_or(Refusal::PanelUnknown)
    }

    pub fn replace(&self, panel: u64, draft: Draft) {
        if let Some(opened) = self.held().get_mut(&panel) {
            opened.draft = draft;
        }
    }

    fn held(&self) -> MutexGuard<'_, HashMap<u64, Opened>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub fn panel_reply(view: PanelView) -> poise::CreateReply {
    poise::CreateReply::default()
        .content(view.content)
        .components(view.components)
        .allowed_mentions(serenity::CreateAllowedMentions::new())
        .ephemeral(true)
}

pub fn weekdays_picked(
    drafts: &Drafts,
    panel: u64,
    user: UserId,
    values: &[String],
    now_ms: u64,
) -> Result<PanelView, Refusal> {
    redrawn(drafts, panel, user, now_ms, |draft| {
        draft.with_weekdays(values)
    })
}

pub fn roles_picked(
    drafts: &Drafts,
    panel: u64,
    user: UserId,
    roles: &[RoleId],
    now_ms: u64,
) -> Result<PanelView, Refusal> {
    redrawn(drafts, panel, user, now_ms, |draft| draft.with_roles(roles))
}

pub fn everyone_pressed(
    drafts: &Drafts,
    panel: u64,
    user: UserId,
    now_ms: u64,
) -> Result<PanelView, Refusal> {
    redrawn(drafts, panel, user, now_ms, Draft::with_everyone)
}

pub fn cancel_pressed(
    drafts: &Drafts,
    panel: u64,
    user: UserId,
    now_ms: u64,
) -> Result<String, Refusal> {
    drafts
        .take(panel, user, now_ms)
        .map(|_| text::PANEL_CANCELLED.to_owned())
}

pub async fn save_pressed<B: Board>(
    drafts: &Drafts,
    panel: u64,
    user: UserId,
    signups: &Arc<Signups<B>>,
    judge: impl FnOnce(&Draft) -> Vec<(Ping, PingVerdict)>,
    now_ms: u64,
) -> SaveAnswer {
    let draft = match drafts.with(panel, user, now_ms) {
        Ok(draft) => draft,
        Err(refusal) => return SaveAnswer::Refused(refusal),
    };
    let now_unix = unix_of(now_ms);
    let verdicts = judge(&draft);
    let answer = match draft.ready(now_unix, &verdicts) {
        Err(refusal) => SaveAnswer::Refused(refusal),
        Ok(ready) => match ready.apply(signups, draft.guild(), now_unix).await {
            Applied::Create {
                outcome: Ok(outcome),
                warning,
            } => match draft.saved_reply(&outcome, warning, now_unix) {
                Ok(reply) => SaveAnswer::Saved(reply),
                Err(refusal) => SaveAnswer::Refused(refusal),
            },
            Applied::Create {
                outcome: Err(error),
                ..
            } => SaveAnswer::Failed {
                summary: "an event could not be saved",
                failure: Failure::from_error(&error),
            },
            Applied::Edit { report, warning } => edit_answer(report, &draft, warning),
        },
    };
    if matches!(answer, SaveAnswer::Saved(_)) {
        let _ = drafts.take(panel, user, now_ms);
    }
    answer
}

fn redrawn(
    drafts: &Drafts,
    panel: u64,
    user: UserId,
    now_ms: u64,
    change: impl FnOnce(&Draft) -> Result<Draft, Refusal>,
) -> Result<PanelView, Refusal> {
    let draft = drafts
        .with(panel, user, now_ms)
        .and_then(|draft| change(&draft))?;
    let view = draft.panel(panel, unix_of(now_ms));
    drafts.replace(panel, draft);
    Ok(view)
}

fn edit_answer(report: EditReport, draft: &Draft, warning: Option<String>) -> SaveAnswer {
    match report {
        EditReport::Edited {
            after,
            cleared,
            refreshed,
            ..
        } => SaveAnswer::Saved(text::warned(
            warning,
            text::event_edited(&after, &cleared, &refreshed),
        )),
        EditReport::ClearFailed { failures } => {
            SaveAnswer::Unsaved(text::failures_noted(text::CLEAR_FAILED_MOVE, failures))
        }
        EditReport::NameTaken => SaveAnswer::Refused(Refusal::NameTaken(draft.name().to_owned())),
        EditReport::BadRange => SaveAnswer::Refused(Refusal::LastDayBeforeFirst),
        EditReport::TooFar => SaveAnswer::Refused(Refusal::DatesTooFar),
        EditReport::NotFound => SaveAnswer::Refused(Refusal::EventNotFound),
        EditReport::Failed(failure) => SaveAnswer::Failed {
            summary: "an event edit could not be saved",
            failure,
        },
    }
}

fn unix_of(now_ms: u64) -> i64 {
    i64::try_from(now_ms / MILLIS_PER_SECOND).unwrap_or(i64::MAX)
}

fn usable(opened: Option<&Opened>, user: UserId, now_ms: u64) -> Result<&Opened, Refusal> {
    let opened = opened.ok_or(Refusal::PanelUnknown)?;
    if opened.draft.owner != user {
        return Err(Refusal::PanelNotYours);
    }
    if expired(opened, now_ms) {
        return Err(Refusal::PanelExpired);
    }
    Ok(opened)
}

fn expired(opened: &Opened, now_ms: u64) -> bool {
    now_ms.saturating_sub(opened.opened_at_ms) > PANEL_LIFETIME_MS
}

pub fn server_zones(events: &[Event], now_unix: i64) -> Vec<String> {
    let newest_first: BTreeMap<Reverse<(u64, i64)>, &Event> = listed_events(events, now_unix)
        .into_iter()
        .map(|event| (Reverse((event.created_at_ms, event.id)), event))
        .collect();
    once_each(
        newest_first
            .into_values()
            .map(|event| event.span.schedule().zone().name().to_owned()),
    )
}

pub fn zone_choices(
    typed: &str,
    read: Result<Vec<Event>, AttendanceError>,
    now_unix: i64,
) -> Vec<String> {
    let zones = read
        .map(|events| server_zones(&events, now_unix))
        .unwrap_or_default();
    zone_suggestions(typed, &zones)
}

pub fn zone_suggestions(typed: &str, server_zones: &[String]) -> Vec<String> {
    let typed = typed.trim();
    if typed.chars().count() > ZONE_NAME_LIMIT {
        return Vec::new();
    }
    let wanted = typed.to_lowercase();
    let accepted = server_zones
        .iter()
        .filter(|name| Zone::find(name).is_some())
        .cloned();
    once_each(accepted.chain(schedule::zone_names()))
        .into_iter()
        .filter(|name| name.to_lowercase().contains(&wanted))
        .take(SUGGESTION_LIMIT)
        .collect()
}

pub fn date_suggestions(typed: &str, today: Date) -> Vec<(String, String)> {
    let wanted = typed.trim();
    let (Ok(first), Ok(last)) = (
        today.checked_sub(1.day()),
        today.checked_add(RANGE_MONTHS.months()),
    ) else {
        return Vec::new();
    };
    first
        .series(1.day())
        .take_while(|date| *date <= last)
        .filter(|date| date.to_string().starts_with(wanted))
        .take(SUGGESTION_LIMIT)
        .map(|date| (text::date_label(date), date.to_string()))
        .collect()
}

fn once_each(names: impl Iterator<Item = String>) -> Vec<String> {
    names.fold(Vec::new(), |kept: Vec<String>, name| {
        if kept.contains(&name) {
            kept
        } else {
            kept.into_iter().chain(std::iter::once(name)).collect()
        }
    })
}

fn everyone_role(guild: GuildId) -> RoleId {
    RoleId::new(guild.get())
}

fn read_start(text: &str) -> Result<StartTime, Refusal> {
    StartTime::parse(text).ok_or(Refusal::StartFormat)
}

fn read_hours(count: i64) -> Result<Hours, Refusal> {
    Hours::new(count).ok_or(Refusal::HoursOutOfRange)
}

fn read_zone(text: &str) -> Result<Zone, Refusal> {
    Zone::find(text).ok_or(Refusal::ZoneUnknown)
}

fn read_day(text: &str) -> Result<Date, Refusal> {
    schedule::parse_day(text.trim()).ok_or(Refusal::DateFormat)
}

fn within_window(span: &Span, now_unix: i64) -> bool {
    schedule::today(now_unix).is_some_and(|today| span.near(today))
}

fn settled_span(
    first_day: Date,
    last_day: Date,
    schedule: Schedule,
    now_unix: i64,
) -> Result<Span, Refusal> {
    let span = Span::new(first_day, last_day, schedule).ok_or(Refusal::LastDayBeforeFirst)?;
    if within_window(&span, now_unix) {
        Ok(span)
    } else {
        Err(Refusal::DatesTooFar)
    }
}

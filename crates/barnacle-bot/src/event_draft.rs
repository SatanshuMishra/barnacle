use std::num::NonZeroU64;
use std::sync::Arc;

use barnacle_guess::UserId;
use jiff::ToSpan;
use jiff::civil::Date;
use jiff::civil::Weekday;
use poise::serenity_prelude as serenity;

use crate::attendance::Board;
use crate::attendance::EditReport;
use crate::attendance::Signups;
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
use crate::panels;
use crate::panels::Owned;
use crate::schedule;
use crate::schedule::Hours;
use crate::schedule::Schedule;
use crate::schedule::Span;
use crate::schedule::StartTime;
use crate::schedule::Weekdays;
use crate::schedule::Zone;
use crate::text;
use crate::text::Refusal;
use crate::wiring;
use crate::wiring::PanelPart;
use crate::wiring::PingChoiceProblem;
use crate::wiring::PingVerdict;
use crate::wiring::SetupPing;

pub use crate::panels::PANEL_LIFETIME_MINUTES;
pub use crate::panels::PANEL_LIFETIME_MS;

pub type Drafts = panels::Drafts<Draft>;

pub const ROLE_LIMIT: usize = ROLE_MENU_MAX as usize;
pub const FIRST_DAY_INPUT: &str = "first";
pub const LAST_DAY_INPUT: &str = "last";
pub const START_INPUT: &str = "start";
pub const END_INPUT: &str = "end";

const MILLIS_PER_SECOND: u64 = 1_000;
const CLOCK_OUT_OF_RANGE: &str = "the clock reads a time outside the supported calendar";
const ROLE_MENU_MAX: u8 = 5;
const DAY_INPUT_MAX: u16 = 32;
const TIME_INPUT_MAX: u16 = 16;
const MINUTES_PER_HOUR: i64 = 60;
const MINUTES_PER_DAY: i64 = 24 * MINUTES_PER_HOUR;
const DAYS_PER_WEEK: usize = 7;
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
const WEEKDAY_NAMES: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
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
    pub template: Option<String>,
    pub zone: Option<Zone>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditOptions {
    pub name: Option<String>,
    pub channel: Option<ChannelId>,
    pub zone: Option<Zone>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Create,
    Edit { event: i64, created_at_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Days,
    Time,
    Pings,
}

impl Step {
    fn number(self) -> usize {
        match self {
            Step::Days => 1,
            Step::Time => 2,
            Step::Pings => 3,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Step::Days => "Days",
            Step::Time => "Time",
            Step::Pings => "Pings",
        }
    }
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
    step: Step,
    first_day: Option<Date>,
    last_day: Option<Date>,
    weekdays: Option<Weekdays>,
    start: Option<StartTime>,
    hours: Option<Hours>,
    zone: Option<Zone>,
    today_zone: Zone,
    everyone: bool,
    roles: Vec<RoleId>,
    changed: Changed,
}

impl Owned for Draft {
    fn owner(&self) -> UserId {
        self.owner
    }
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
        _now_unix: i64,
    ) -> Result<Draft, Refusal> {
        let name = wiring::event_name(&options.name).ok_or(Refusal::NameFormat)?;
        let template = options
            .template
            .as_deref()
            .map(|text| Template::named(text).ok_or(Refusal::TemplateUnknown))
            .transpose()?
            .map(Template::schedule);
        let zone = match &template {
            Some(base) => base.zone().clone(),
            None => options.zone.clone().ok_or(Refusal::ZoneNeeded)?,
        };
        Ok(Draft {
            purpose: Purpose::Create,
            owner,
            guild,
            channel,
            name,
            step: Step::Days,
            first_day: None,
            last_day: None,
            weekdays: template.as_ref().map(Schedule::weekdays),
            start: template.as_ref().map(Schedule::start),
            hours: template.as_ref().map(Schedule::hours),
            today_zone: options.zone.clone().unwrap_or_else(|| zone.clone()),
            zone: Some(zone),
            everyone: false,
            roles: Vec::new(),
            changed: Changed::default(),
        })
    }

    pub fn edit(
        event: &Event,
        options: &EditOptions,
        owner: UserId,
        _now_unix: i64,
    ) -> Result<Draft, Refusal> {
        let held = event.span.schedule();
        let name = options
            .name
            .as_deref()
            .map(|text| wiring::event_name(text).ok_or(Refusal::NameFormat))
            .transpose()?;
        let everyone = everyone_role(event.guild);
        let zone = options.zone.clone().unwrap_or_else(|| held.zone().clone());
        Ok(Draft {
            purpose: Purpose::Edit {
                event: event.id,
                created_at_ms: event.created_at_ms,
            },
            owner,
            guild: event.guild,
            channel: options.channel.unwrap_or(event.channel),
            changed: Changed {
                name: name.is_some(),
                channel: options.channel.is_some(),
                zone: zone != *held.zone(),
                ..Changed::default()
            },
            name: name.unwrap_or_else(|| event.name.clone()),
            step: Step::Days,
            first_day: Some(event.span.first_day()),
            last_day: Some(event.span.last_day()),
            weekdays: Some(held.weekdays()),
            start: Some(held.start()),
            hours: Some(held.hours()),
            today_zone: zone.clone(),
            zone: Some(zone),
            everyone: event.ping_roles.contains(&everyone),
            roles: event
                .ping_roles
                .iter()
                .copied()
                .filter(|role| *role != everyone)
                .collect(),
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

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn first_day(&self) -> Option<Date> {
        self.first_day
    }

    pub fn last_day(&self) -> Option<Date> {
        self.last_day
    }

    pub fn weekdays(&self) -> Option<Weekdays> {
        self.weekdays
    }

    pub fn start(&self) -> Option<StartTime> {
        self.start
    }

    pub fn hours(&self) -> Option<Hours> {
        self.hours
    }

    pub fn zone(&self) -> Option<&Zone> {
        self.zone.as_ref()
    }

    pub fn span(&self) -> Option<Span> {
        let (first_day, last_day, weekdays) = self.picked_days().ok()?;
        let (start, hours, zone) = self.picked_time().ok()?;
        Span::new(
            first_day,
            last_day,
            Schedule::new(weekdays, start, zone, hours),
        )
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

    pub fn with_typed_dates(
        &self,
        first: &str,
        last: &str,
        now_unix: i64,
    ) -> Result<Draft, Refusal> {
        let first_day = read_day(first)?;
        let last_day = read_day(last)?;
        let today = self
            .today_zone
            .today(now_unix)
            .ok_or(Refusal::DatesTooFar)?;
        if self.purpose == Purpose::Create && first_day < today {
            return Err(Refusal::FirstDayPast);
        }
        if last_day < first_day {
            return Err(Refusal::LastDayBeforeFirst);
        }
        let typed = Span::new(first_day, last_day, Schedule::clan_battles())
            .ok_or(Refusal::LastDayBeforeFirst)?;
        if !within_window(&typed, now_unix) {
            return Err(Refusal::DatesTooFar);
        }
        let offered = weekdays_between(first_day, last_day);
        let weekdays = self.weekdays.and_then(|picked| {
            Weekdays::new(
                &offered
                    .iter()
                    .copied()
                    .filter(|day| picked.contains(*day))
                    .collect::<Vec<Weekday>>(),
            )
        });
        Ok(Draft {
            first_day: Some(first_day),
            last_day: Some(last_day),
            weekdays,
            changed: Changed {
                first_day: self.changed.first_day || self.first_day != Some(first_day),
                last_day: self.changed.last_day || self.last_day != Some(last_day),
                weekdays: self.changed.weekdays || self.weekdays != weekdays,
                ..self.changed
            },
            ..self.clone()
        })
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
        Ok(Draft {
            weekdays: Some(weekdays),
            changed: Changed {
                weekdays: self.changed.weekdays || self.weekdays != Some(weekdays),
                ..self.changed
            },
            ..self.clone()
        })
    }

    pub fn with_typed_time(&self, start: &str, end: &str) -> Result<Draft, Refusal> {
        let start = StartTime::parse(start).ok_or(Refusal::TimeFormat)?;
        let end = StartTime::parse(end).ok_or(Refusal::TimeFormat)?;
        let length = match (end.minutes() - start.minutes()).rem_euclid(MINUTES_PER_DAY) {
            0 => MINUTES_PER_DAY,
            minutes => minutes,
        };
        let hours = (length % MINUTES_PER_HOUR == 0)
            .then(|| Hours::new(length / MINUTES_PER_HOUR))
            .flatten()
            .ok_or(Refusal::LengthNotWholeHours)?;
        Ok(Draft {
            start: Some(start),
            hours: Some(hours),
            changed: Changed {
                start: self.changed.start || self.start != Some(start),
                hours: self.changed.hours || self.hours != Some(hours),
                ..self.changed
            },
            ..self.clone()
        })
    }

    pub fn time_modal(&self, panel: u64) -> serenity::CreateModal {
        let end = self.start.zip(self.hours).and_then(|(start, hours)| {
            StartTime::from_minutes(
                (start.minutes() + i64::from(hours.get()) * MINUTES_PER_HOUR)
                    .rem_euclid(MINUTES_PER_DAY),
            )
        });
        serenity::CreateModal::new(
            wiring::panel_id(panel, PanelPart::TimeModal),
            text::TIME_MODAL_TITLE,
        )
        .components(vec![
            time_input(text::START_INPUT_LABEL, START_INPUT, self.start),
            time_input(text::END_INPUT_LABEL, END_INPUT, end),
        ])
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

    pub fn next(&self) -> Result<Draft, Refusal> {
        let step = match self.step {
            Step::Days => self.picked_days().map(|_| Step::Time)?,
            Step::Time => self.picked_time().map(|_| Step::Pings)?,
            Step::Pings => Step::Pings,
        };
        Ok(Draft {
            step,
            ..self.clone()
        })
    }

    pub fn back(&self) -> Draft {
        let step = match self.step {
            Step::Days | Step::Time => Step::Days,
            Step::Pings => Step::Time,
        };
        Draft {
            step,
            ..self.clone()
        }
    }

    pub fn panel(&self, panel: u64, now_unix: i64) -> PanelView {
        let components = match self.step {
            Step::Days => self.days_rows(panel),
            Step::Time => self.time_rows(panel),
            Step::Pings => self.pings_rows(panel, now_unix),
        };
        PanelView {
            content: self.content(now_unix),
            components,
        }
    }

    pub fn ready(&self, now_unix: i64, verdicts: &[(Ping, PingVerdict)]) -> Result<Ready, Refusal> {
        let creating = self.purpose == Purpose::Create;
        if !creating && !self.changed.any() {
            return Err(Refusal::NothingToChange);
        }
        let (first_day, last_day, weekdays) = self.picked_days()?;
        let (start, hours, zone) = self.picked_time()?;
        let span = Span::new(
            first_day,
            last_day,
            Schedule::new(weekdays, start, zone, hours),
        )
        .ok_or(Refusal::LastDayBeforeFirst)?;
        if !within_window(&span, now_unix) {
            return Err(Refusal::DatesTooFar);
        }
        if self.sessions_matter() && span.sessions_left(now_unix) == 0 {
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
                    span,
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
                change: self.change(&span),
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

    fn change(&self, span: &Span) -> EventChange {
        let schedule = span.schedule();
        EventChange {
            name: self.changed.name.then(|| self.name.clone()),
            first_day: self.changed.first_day.then(|| span.first_day()),
            last_day: self.changed.last_day.then(|| span.last_day()),
            weekdays: self.changed.weekdays.then(|| schedule.weekdays()),
            start: self.changed.start.then(|| schedule.start()),
            zone: self.changed.zone.then(|| schedule.zone().clone()),
            hours: self.changed.hours.then(|| schedule.hours()),
            channel: self.changed.channel.then_some(self.channel),
            ping_roles: self.changed.pings.then(|| self.ping_roles()),
        }
    }

    fn picked_days(&self) -> Result<(Date, Date, Weekdays), Refusal> {
        match (self.first_day, self.last_day, self.weekdays) {
            (Some(first_day), Some(last_day), Some(weekdays)) => {
                if last_day < first_day {
                    Err(Refusal::LastDayBeforeFirst)
                } else if weekday_count(first_day, last_day, weekdays) == 0 {
                    Err(Refusal::WeekdaysInvalid)
                } else {
                    Ok((first_day, last_day, weekdays))
                }
            }
            (first_day, last_day, weekdays) => Err(missing(&[
                ("the dates", first_day.is_none() || last_day.is_none()),
                ("the event days", weekdays.is_none()),
            ])),
        }
    }

    fn picked_time(&self) -> Result<(StartTime, Hours, Zone), Refusal> {
        match (self.start, self.hours, &self.zone) {
            (Some(start), Some(hours), Some(zone)) => Ok((start, hours, zone.clone())),
            (start, hours, zone) => Err(missing(&[
                ("the session time", start.is_none() || hours.is_none()),
                ("the time zone", zone.is_none()),
            ])),
        }
    }

    fn sessions_matter(&self) -> bool {
        self.purpose == Purpose::Create || self.changed.schedule()
    }

    fn nothing_ahead(&self, now_unix: i64) -> bool {
        self.span()
            .is_some_and(|span| self.sessions_matter() && span.sessions_left(now_unix) == 0)
    }

    fn content(&self, now_unix: i64) -> String {
        let session_count = self.picked_days().map_or(0, |(first, last, weekdays)| {
            weekday_count(first, last, weekdays)
        });
        let time = self
            .picked_time()
            .ok()
            .map(|(start, hours, zone)| text::time_line(start, zone.name(), hours));
        let first_session = self
            .span()
            .and_then(|span| text::first_session_line(&span, now_unix));
        let pings = (self.step == Step::Pings).then(|| text::pings_summary(&self.pings()));
        [
            Some(text::panel_heading(&self.name, self.channel)),
            Some(text::step_line(self.step.number(), self.step.name())),
            self.zone.as_ref().map(|zone| text::zone_line(zone.name())),
            Some(text::days_line(
                self.first_day,
                self.last_day,
                self.weekdays,
                session_count,
            )),
            time,
            first_session,
            pings,
            self.problem(now_unix).map(str::to_owned),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<String>>()
        .join("\n")
    }

    fn problem(&self, now_unix: i64) -> Option<&'static str> {
        match (self.first_day, self.last_day, self.weekdays) {
            (Some(first), Some(last), _) if last < first => Some(text::LAST_DAY_BEFORE_FIRST),
            (Some(first), Some(last), Some(weekdays))
                if weekday_count(first, last, weekdays) == 0 =>
            {
                Some(text::NO_WEEKDAY_IN_DATES)
            }
            _ if self.step == Step::Pings && self.nothing_ahead(now_unix) => {
                Some(text::EVERY_SESSION_STARTED)
            }
            _ => None,
        }
    }

    fn days_rows(&self, panel: u64) -> Vec<serenity::CreateActionRow> {
        let buttons = serenity::CreateActionRow::Buttons(vec![
            serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::TypeDates))
                .label(text::SET_DATES_LABEL)
                .style(serenity::ButtonStyle::Secondary),
            serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::Next))
                .label(text::NEXT_TIME_LABEL)
                .style(serenity::ButtonStyle::Primary)
                .disabled(self.picked_days().is_err()),
            cancel_button(panel),
        ]);
        let Some(offered) = self
            .first_day
            .zip(self.last_day)
            .map(|(first, last)| weekdays_between(first, last))
            .filter(|offered| !offered.is_empty())
        else {
            return vec![buttons];
        };
        let picked = self.weekdays;
        let most = u8::try_from(offered.len()).unwrap_or(u8::MAX);
        let days = serenity::CreateSelectMenu::new(
            wiring::panel_id(panel, PanelPart::Days),
            serenity::CreateSelectMenuKind::String {
                options: WEEKDAY_VALUES
                    .iter()
                    .filter(|(day, _)| offered.contains(day))
                    .map(|&(day, value)| {
                        serenity::CreateSelectMenuOption::new(text::weekday_name(day), value)
                            .default_selection(picked.is_some_and(|picked| picked.contains(day)))
                    })
                    .collect(),
            },
        )
        .placeholder(text::EVENT_DAYS_PLACEHOLDER)
        .min_values(1)
        .max_values(most);
        vec![serenity::CreateActionRow::SelectMenu(days), buttons]
    }

    fn time_rows(&self, panel: u64) -> Vec<serenity::CreateActionRow> {
        vec![serenity::CreateActionRow::Buttons(vec![
            back_button(panel),
            serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::SetTime))
                .label(text::SET_TIME_LABEL)
                .style(serenity::ButtonStyle::Secondary),
            serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::Next))
                .label(text::NEXT_PINGS_LABEL)
                .style(serenity::ButtonStyle::Primary)
                .disabled(self.picked_time().is_err()),
            cancel_button(panel),
        ])]
    }

    fn pings_rows(&self, panel: u64, now_unix: i64) -> Vec<serenity::CreateActionRow> {
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
        let save_blocked = self.span().is_none() || self.nothing_ahead(now_unix);
        vec![
            serenity::CreateActionRow::SelectMenu(roles),
            serenity::CreateActionRow::Buttons(vec![
                back_button(panel),
                serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::Save))
                    .label(save)
                    .style(serenity::ButtonStyle::Success)
                    .disabled(save_blocked),
                serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::Everyone))
                    .label(text::everyone_label(self.everyone))
                    .style(everyone_style),
                cancel_button(panel),
            ]),
        ]
    }
}

pub fn dates_modal(panel: u64) -> serenity::CreateModal {
    serenity::CreateModal::new(
        wiring::panel_id(panel, PanelPart::DatesModal),
        text::DATES_MODAL_TITLE,
    )
    .components(vec![
        day_input(
            text::FIRST_DAY_INPUT_LABEL,
            FIRST_DAY_INPUT,
            text::FIRST_DAY_EXAMPLE,
        ),
        day_input(
            text::LAST_DAY_INPUT_LABEL,
            LAST_DAY_INPUT,
            text::LAST_DAY_EXAMPLE,
        ),
    ])
}

pub fn day_input(label: &str, custom_id: &str, example: &str) -> serenity::CreateActionRow {
    serenity::CreateActionRow::InputText(
        serenity::CreateInputText::new(serenity::InputTextStyle::Short, label, custom_id)
            .placeholder(example)
            .max_length(DAY_INPUT_MAX),
    )
}

fn time_input(label: &str, custom_id: &str, value: Option<StartTime>) -> serenity::CreateActionRow {
    let input = serenity::CreateInputText::new(serenity::InputTextStyle::Short, label, custom_id)
        .placeholder(text::TIME_EXAMPLE)
        .max_length(TIME_INPUT_MAX);
    serenity::CreateActionRow::InputText(match value {
        Some(time) => input.value(time.to_string()),
        None => input,
    })
}

fn back_button(panel: u64) -> serenity::CreateButton {
    serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::Back))
        .label(text::BACK_LABEL)
        .style(serenity::ButtonStyle::Secondary)
}

fn cancel_button(panel: u64) -> serenity::CreateButton {
    serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::Cancel))
        .label(text::CANCEL_LABEL)
        .style(serenity::ButtonStyle::Secondary)
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

pub fn dates_typed(
    drafts: &Drafts,
    panel: u64,
    user: UserId,
    first: &str,
    last: &str,
    now_ms: u64,
) -> Result<PanelView, Refusal> {
    redrawn(drafts, panel, user, now_ms, |draft| {
        draft.with_typed_dates(first, last, unix_of(now_ms))
    })
}

pub fn time_typed(
    drafts: &Drafts,
    panel: u64,
    user: UserId,
    start: &str,
    end: &str,
    now_ms: u64,
) -> Result<PanelView, Refusal> {
    redrawn(drafts, panel, user, now_ms, |draft| {
        draft.with_typed_time(start, end)
    })
}

pub fn time_modal_opened(
    drafts: &Drafts,
    panel: u64,
    user: UserId,
    now_ms: u64,
) -> Result<serenity::CreateModal, Refusal> {
    drafts
        .with(panel, user, now_ms)
        .map(|draft| draft.time_modal(panel))
}

pub fn modal_opened(
    drafts: &Drafts,
    panel: u64,
    user: UserId,
    now_ms: u64,
    modal: impl FnOnce(u64) -> serenity::CreateModal,
) -> Result<serenity::CreateModal, Refusal> {
    drafts.with(panel, user, now_ms).map(|_| modal(panel))
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

pub fn redrawn(
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

fn everyone_role(guild: GuildId) -> RoleId {
    RoleId::new(guild.get())
}

fn missing(choices: &[(&'static str, bool)]) -> Refusal {
    Refusal::MissingOptions(
        choices
            .iter()
            .filter_map(|(name, absent)| absent.then_some(*name))
            .collect(),
    )
}

fn weekday_count(first_day: Date, last_day: Date, weekdays: Weekdays) -> usize {
    first_day
        .series(1.day())
        .take_while(|day| *day <= last_day)
        .filter(|day| weekdays.contains(day.weekday()))
        .count()
}

fn weekdays_between(first_day: Date, last_day: Date) -> Vec<Weekday> {
    let occurring: Vec<Weekday> = first_day
        .series(1.day())
        .take_while(|day| *day <= last_day)
        .take(DAYS_PER_WEEK)
        .map(|day| day.weekday())
        .collect();
    WEEKDAY_VALUES
        .iter()
        .map(|(day, _)| *day)
        .filter(|day| occurring.contains(day))
        .collect()
}

pub(crate) fn within_window(span: &Span, now_unix: i64) -> bool {
    schedule::today(now_unix).is_some_and(|today| span.near(today))
}

pub(crate) fn read_day(text: &str) -> Result<Date, Refusal> {
    schedule::parse_day(without_weekday(text.trim())).ok_or(Refusal::DateFormat)
}

fn without_weekday(text: &str) -> &str {
    let Some((day, bracketed)) = text.split_once('(') else {
        return text;
    };
    let named = bracketed
        .strip_suffix(')')
        .map(str::trim)
        .is_some_and(|name| {
            WEEKDAY_NAMES
                .iter()
                .any(|weekday| weekday.eq_ignore_ascii_case(name))
        });
    if named { day.trim_end() } else { text }
}

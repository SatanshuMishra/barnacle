use std::cmp::Reverse;
use std::collections::BTreeMap;
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
use crate::panels;
use crate::panels::Owned;
use crate::schedule;
use crate::schedule::Hours;
use crate::schedule::MAX_HOURS;
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
use crate::wiring::SetupPing;

pub use crate::panels::PANEL_LIFETIME_MINUTES;
pub use crate::panels::PANEL_LIFETIME_MS;

pub type Drafts = panels::Drafts<Draft>;

pub const ROLE_LIMIT: usize = ROLE_MENU_MAX as usize;
pub const ANOTHER_ZONE: &str = "other";
pub const FIRST_DAY_INPUT: &str = "first";
pub const LAST_DAY_INPUT: &str = "last";
pub const ZONE_INPUT: &str = "zone";

const MILLIS_PER_SECOND: u64 = 1_000;
const CLOCK_OUT_OF_RANGE: &str = "the clock reads a time outside the supported calendar";
const ROLE_MENU_MAX: u8 = 5;
const WEEKDAY_MENU_MAX: u8 = 7;
const MENU_OPTIONS: usize = 25;
const ZONE_MENU_ZONES: usize = MENU_OPTIONS - 1;
const DAY_INPUT_MAX: u16 = 32;
const ZONE_INPUT_MAX: u16 = ZONE_NAME_LIMIT as u16;
const HOURS_OF_DAY: std::ops::Range<i8> = 0..24;
const QUARTER_MINUTES: [i8; 4] = [0, 15, 30, 45];
const CLOCK_DIGITS: usize = 2;
const CLAN_BATTLES: &str = "Clan Battles";
const COMMON_ZONES: [&str; 9] = [
    "America/Vancouver",
    "America/Edmonton",
    "America/Denver",
    "America/Chicago",
    "America/Toronto",
    "America/New_York",
    "Europe/London",
    "Europe/Berlin",
    "UTC",
];
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
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditOptions {
    pub name: Option<String>,
    pub channel: Option<ChannelId>,
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
    hour: Option<i8>,
    minute: Option<i8>,
    hours: Option<Hours>,
    zone: Option<Zone>,
    own_minute: Option<i8>,
    server_zones: Vec<String>,
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
            hour: template.as_ref().map(|base| base.start().hour()),
            minute: template.as_ref().map(|base| base.start().minute()),
            hours: template.as_ref().map(Schedule::hours),
            zone: template.as_ref().map(|base| base.zone().clone()),
            own_minute: None,
            server_zones: Vec::new(),
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
                ..Changed::default()
            },
            name: name.unwrap_or_else(|| event.name.clone()),
            step: Step::Days,
            first_day: Some(event.span.first_day()),
            last_day: Some(event.span.last_day()),
            weekdays: Some(held.weekdays()),
            hour: Some(held.start().hour()),
            minute: Some(held.start().minute()),
            hours: Some(held.hours()),
            zone: Some(held.zone().clone()),
            own_minute: Some(held.start().minute()),
            server_zones: Vec::new(),
            everyone: event.ping_roles.contains(&everyone),
            roles: event
                .ping_roles
                .iter()
                .copied()
                .filter(|role| *role != everyone)
                .collect(),
        })
    }

    pub fn with_server_zones(&self, zones: Vec<String>) -> Draft {
        Draft {
            server_zones: zones,
            ..self.clone()
        }
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
        StartTime::new(self.hour?, self.minute?)
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

    pub fn with_first_day(&self, value: &str) -> Result<Draft, Refusal> {
        let first_day = read_day(value)?;
        let last_day = self.last_day.filter(|last| *last >= first_day);
        Ok(Draft {
            first_day: Some(first_day),
            last_day,
            changed: Changed {
                first_day: self.changed.first_day || self.first_day != Some(first_day),
                last_day: self.changed.last_day || self.last_day != last_day,
                ..self.changed
            },
            ..self.clone()
        })
    }

    pub fn with_last_day(&self, value: &str) -> Result<Draft, Refusal> {
        let last_day = read_day(value)?;
        if self.first_day.is_some_and(|first| last_day < first) {
            return Err(Refusal::LastDayBeforeFirst);
        }
        Ok(Draft {
            last_day: Some(last_day),
            changed: Changed {
                last_day: self.changed.last_day || self.last_day != Some(last_day),
                ..self.changed
            },
            ..self.clone()
        })
    }

    pub fn with_typed_dates(
        &self,
        first: &str,
        last: &str,
        now_unix: i64,
    ) -> Result<Draft, Refusal> {
        let first_day = read_day(first)?;
        let last_day = read_day(last)?;
        let today = zone_today(&self.server_zones, now_unix).ok_or(Refusal::DatesTooFar)?;
        if self.purpose == Purpose::Create && first_day < today {
            return Err(Refusal::FirstDayPast);
        }
        if last_day < first_day {
            return Err(Refusal::LastDayBeforeFirst);
        }
        if !within_window_of(first_day, last_day, today) {
            return Err(Refusal::DatesTooFar);
        }
        Ok(Draft {
            first_day: Some(first_day),
            last_day: Some(last_day),
            changed: Changed {
                first_day: self.changed.first_day || self.first_day != Some(first_day),
                last_day: self.changed.last_day || self.last_day != Some(last_day),
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

    pub fn with_hour(&self, value: &str) -> Result<Draft, Refusal> {
        let hour = read_clock(value)
            .filter(|hour| HOURS_OF_DAY.contains(hour))
            .ok_or(Refusal::StartFormat)?;
        Ok(Draft {
            hour: Some(hour),
            changed: Changed {
                start: self.changed.start || self.hour != Some(hour),
                ..self.changed
            },
            ..self.clone()
        })
    }

    pub fn with_minute(&self, value: &str) -> Result<Draft, Refusal> {
        let minute = read_clock(value)
            .filter(|minute| self.minute_choices().contains(minute))
            .ok_or(Refusal::StartFormat)?;
        Ok(Draft {
            minute: Some(minute),
            changed: Changed {
                start: self.changed.start || self.minute != Some(minute),
                ..self.changed
            },
            ..self.clone()
        })
    }

    pub fn with_hours(&self, value: &str) -> Result<Draft, Refusal> {
        let hours = read_count(value)
            .and_then(|count| Hours::new(i64::from(count)))
            .ok_or(Refusal::HoursOutOfRange)?;
        Ok(Draft {
            hours: Some(hours),
            changed: Changed {
                hours: self.changed.hours || self.hours != Some(hours),
                ..self.changed
            },
            ..self.clone()
        })
    }

    pub fn with_zone(&self, name: &str) -> Result<Draft, Refusal> {
        let zone = Zone::find(name).ok_or(Refusal::ZoneUnknown)?;
        Ok(Draft {
            changed: Changed {
                zone: self.changed.zone || self.zone.as_ref() != Some(&zone),
                ..self.changed
            },
            zone: Some(zone),
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
            Step::Days => self.days_rows(panel, now_unix),
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
                ("the first day", first_day.is_none()),
                ("the last day", last_day.is_none()),
                ("the days of the week", weekdays.is_none()),
            ])),
        }
    }

    fn picked_time(&self) -> Result<(StartTime, Hours, Zone), Refusal> {
        match (self.hour, self.minute, self.hours, &self.zone) {
            (Some(hour), Some(minute), Some(hours), Some(zone)) => StartTime::new(hour, minute)
                .map(|start| (start, hours, zone.clone()))
                .ok_or(Refusal::StartFormat),
            (hour, minute, hours, zone) => Err(missing(&[
                ("the start hour", hour.is_none()),
                ("the start minute", minute.is_none()),
                ("the length", hours.is_none()),
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

    fn days_rows(&self, panel: u64, now_unix: i64) -> Vec<serenity::CreateActionRow> {
        let today = zone_today(&self.server_zones, now_unix)
            .or(self.first_day)
            .unwrap_or(Date::ZERO);
        let first_days = string_menu(
            panel,
            PanelPart::FirstDay,
            text::FIRST_DAY_PLACEHOLDER,
            day_options(today, today, self.first_day),
        );
        let last_days = string_menu(
            panel,
            PanelPart::LastDay,
            text::LAST_DAY_PLACEHOLDER,
            day_options(self.first_day.unwrap_or(today), today, self.last_day),
        );
        let weekdays = self.weekdays;
        let days = serenity::CreateSelectMenu::new(
            wiring::panel_id(panel, PanelPart::Days),
            serenity::CreateSelectMenuKind::String {
                options: WEEKDAY_VALUES
                    .iter()
                    .map(|(day, value)| {
                        serenity::CreateSelectMenuOption::new(text::weekday_name(*day), *value)
                            .default_selection(weekdays.is_some_and(|picked| picked.contains(*day)))
                    })
                    .collect(),
            },
        )
        .placeholder(text::WEEKDAYS_PLACEHOLDER)
        .min_values(1)
        .max_values(WEEKDAY_MENU_MAX);
        vec![
            first_days,
            last_days,
            serenity::CreateActionRow::SelectMenu(days),
            serenity::CreateActionRow::Buttons(vec![
                serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::Next))
                    .label(text::NEXT_TIME_LABEL)
                    .style(serenity::ButtonStyle::Primary)
                    .disabled(self.picked_days().is_err()),
                serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::TypeDates))
                    .label(text::TYPE_DATES_LABEL)
                    .style(serenity::ButtonStyle::Secondary),
                cancel_button(panel),
            ]),
        ]
    }

    fn time_rows(&self, panel: u64) -> Vec<serenity::CreateActionRow> {
        let hours = HOURS_OF_DAY
            .map(|hour| {
                serenity::CreateSelectMenuOption::new(text::hour_label(hour), clock_value(hour))
                    .description(text::hour_description(hour))
                    .default_selection(self.hour == Some(hour))
            })
            .collect();
        let minutes = self
            .minute_choices()
            .into_iter()
            .map(|minute| {
                serenity::CreateSelectMenuOption::new(
                    text::minute_label(minute),
                    clock_value(minute),
                )
                .default_selection(self.minute == Some(minute))
            })
            .collect();
        let lengths = (1..=MAX_HOURS)
            .filter_map(|count| Hours::new(i64::from(count)))
            .map(|length| {
                serenity::CreateSelectMenuOption::new(
                    text::length_label(length),
                    length.get().to_string(),
                )
                .default_selection(self.hours == Some(length))
            })
            .collect();
        vec![
            string_menu(panel, PanelPart::Hour, text::HOUR_PLACEHOLDER, hours),
            string_menu(panel, PanelPart::Minute, text::MINUTE_PLACEHOLDER, minutes),
            string_menu(panel, PanelPart::Length, text::LENGTH_PLACEHOLDER, lengths),
            string_menu(
                panel,
                PanelPart::Zone,
                text::ZONE_PLACEHOLDER,
                zone_options(&self.server_zones, self.zone.as_ref().map(Zone::name)),
            ),
            serenity::CreateActionRow::Buttons(vec![
                back_button(panel),
                serenity::CreateButton::new(wiring::panel_id(panel, PanelPart::Next))
                    .label(text::NEXT_PINGS_LABEL)
                    .style(serenity::ButtonStyle::Primary)
                    .disabled(self.picked_time().is_err()),
                cancel_button(panel),
            ]),
        ]
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

    fn minute_choices(&self) -> Vec<i8> {
        let mut minutes: Vec<i8> = QUARTER_MINUTES.into_iter().chain(self.own_minute).collect();
        minutes.sort_unstable();
        minutes.dedup();
        minutes
    }
}

pub fn day_options(
    start: Date,
    today: Date,
    picked: Option<Date>,
) -> Vec<serenity::CreateSelectMenuOption> {
    let generated: Vec<Date> = start.series(1.day()).take(MENU_OPTIONS).collect();
    let days: Vec<Date> = match picked {
        Some(day) if !generated.contains(&day) => std::iter::once(day)
            .chain(generated.into_iter().take(MENU_OPTIONS - 1))
            .collect(),
        _ => generated,
    };
    days.into_iter()
        .map(|day| {
            serenity::CreateSelectMenuOption::new(text::day_label(day), day.to_string())
                .description(text::day_offset(days_between(today, day)))
                .default_selection(picked == Some(day))
        })
        .collect()
}

pub fn zone_options(
    server_zones: &[String],
    picked: Option<&str>,
) -> Vec<serenity::CreateSelectMenuOption> {
    let listed = once_each(
        server_zones
            .iter()
            .filter_map(|name| Zone::find(name))
            .map(|zone| zone.name().to_owned())
            .chain(COMMON_ZONES.iter().map(|name| (*name).to_owned())),
    );
    let zones: Vec<String> = match picked {
        Some(zone) if !listed.iter().any(|name| name == zone) => {
            std::iter::once(zone.to_owned()).chain(listed).collect()
        }
        _ => listed,
    };
    zones
        .into_iter()
        .take(ZONE_MENU_ZONES)
        .map(|name| {
            let chosen = picked == Some(name.as_str());
            serenity::CreateSelectMenuOption::new(name.clone(), name).default_selection(chosen)
        })
        .chain(std::iter::once(
            serenity::CreateSelectMenuOption::new(text::ANOTHER_ZONE_LABEL, ANOTHER_ZONE)
                .description(text::ANOTHER_ZONE_DESCRIPTION),
        ))
        .collect()
}

pub fn dates_modal(panel: u64) -> serenity::CreateModal {
    serenity::CreateModal::new(
        wiring::panel_id(panel, PanelPart::DatesModal),
        text::DATES_MODAL_TITLE,
    )
    .components(vec![
        text_input(
            text::FIRST_DAY_INPUT_LABEL,
            FIRST_DAY_INPUT,
            text::FIRST_DAY_EXAMPLE,
            DAY_INPUT_MAX,
        ),
        text_input(
            text::LAST_DAY_INPUT_LABEL,
            LAST_DAY_INPUT,
            text::LAST_DAY_EXAMPLE,
            DAY_INPUT_MAX,
        ),
    ])
}

pub fn zone_modal(panel: u64) -> serenity::CreateModal {
    serenity::CreateModal::new(
        wiring::panel_id(panel, PanelPart::ZoneModal),
        text::ZONE_MODAL_TITLE,
    )
    .components(vec![serenity::CreateActionRow::InputText(
        serenity::CreateInputText::new(
            serenity::InputTextStyle::Short,
            text::ZONE_INPUT_LABEL,
            ZONE_INPUT,
        )
        .max_length(ZONE_INPUT_MAX),
    )])
}

fn text_input(
    label: &str,
    custom_id: &str,
    example: &str,
    max_length: u16,
) -> serenity::CreateActionRow {
    serenity::CreateActionRow::InputText(
        serenity::CreateInputText::new(serenity::InputTextStyle::Short, label, custom_id)
            .placeholder(example)
            .max_length(max_length),
    )
}

fn string_menu(
    panel: u64,
    part: PanelPart,
    placeholder: &str,
    options: Vec<serenity::CreateSelectMenuOption>,
) -> serenity::CreateActionRow {
    serenity::CreateActionRow::SelectMenu(
        serenity::CreateSelectMenu::new(
            wiring::panel_id(panel, part),
            serenity::CreateSelectMenuKind::String { options },
        )
        .placeholder(placeholder),
    )
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

fn days_between(from: Date, to: Date) -> i32 {
    from.until(to).map_or(0, |span| span.get_days())
}

fn zone_today(server_zones: &[String], now_unix: i64) -> Option<Date> {
    let zone = server_zones
        .first()
        .and_then(|name| Zone::find(name))
        .unwrap_or_else(Zone::utc);
    let midnight = Schedule::new(
        Weekdays::EVERY_DAY,
        StartTime::new(0, 0)?,
        zone,
        Hours::new(1)?,
    );
    let utc_today = schedule::today(now_unix)?;
    [1, 0, -1]
        .into_iter()
        .filter_map(|offset: i64| utc_today.checked_add(offset.days()).ok())
        .find(|day| {
            midnight
                .session_at(*day)
                .is_some_and(|session| session.start_unix() <= now_unix)
        })
}

fn within_window_of(first_day: Date, last_day: Date, today: Date) -> bool {
    Span::new(first_day, last_day, Schedule::clan_battles()).is_some_and(|span| span.near(today))
}

fn within_window(span: &Span, now_unix: i64) -> bool {
    schedule::today(now_unix).is_some_and(|today| span.near(today))
}

fn clock_value(value: i8) -> String {
    format!("{value:02}")
}

fn read_clock(text: &str) -> Option<i8> {
    let text = text.trim();
    (text.len() == CLOCK_DIGITS)
        .then(|| read_count(text))
        .flatten()
}

fn read_count(text: &str) -> Option<i8> {
    let text = text.trim();
    if text.is_empty()
        || text.len() > CLOCK_DIGITS
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    text.parse().ok()
}

fn read_day(text: &str) -> Result<Date, Refusal> {
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

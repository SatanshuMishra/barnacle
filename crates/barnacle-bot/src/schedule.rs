use std::collections::BTreeSet;
use std::fmt;

use jiff::ToSpan;
use jiff::civil::Date;
use jiff::civil::Weekday;
use jiff::civil::time;
use jiff::tz::Offset;
use jiff::tz::TimeZone;

pub const MAX_HOURS: u8 = 24;
pub const SECONDS_PER_HOUR: i64 = 3600;
pub const POST_LEAD_SECONDS: i64 = 24 * SECONDS_PER_HOUR;
pub const REMOVE_AFTER_END_SECONDS: i64 = 30 * 60;

pub const RANGE_MONTHS: i32 = 6;
pub const REHEARSAL_SESSIONS: u32 = 3;
pub const REHEARSAL_SESSIONS_MAX: u32 = 7;

pub const ZONE_NAME_LIMIT: usize = 64;

const DAY_TEXT_LENGTH: usize = 10;
const UTC_NAME: &str = "UTC";
const MINUTES_PER_HOUR: i64 = 60;
const MINUTES_PER_DAY: i64 = 24 * MINUTES_PER_HOUR;
const LAST_HOUR_OF_DAY: i8 = 23;
const LAST_MINUTE_OF_HOUR: i8 = 59;
const ALL_WEEKDAY_BITS: u8 = 127;
const WEEK: [Weekday; 7] = [
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
    Weekday::Saturday,
    Weekday::Sunday,
];
const CLAN_BATTLES_START: StartTime = StartTime {
    hour: 23,
    minute: 30,
};
const CLAN_BATTLES_HOURS: Hours = Hours(4);

pub fn today(now_unix: i64) -> Option<Date> {
    let stamp = jiff::Timestamp::from_second(now_unix).ok()?;
    Some(Offset::UTC.to_datetime(stamp).date())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hour(u8);

impl Hour {
    pub fn new(value: u8) -> Option<Self> {
        (1..=MAX_HOURS).contains(&value).then_some(Self(value))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hours(u8);

impl Hours {
    pub fn new(count: i64) -> Option<Self> {
        u8::try_from(count)
            .ok()
            .filter(|count| (1..=MAX_HOURS).contains(count))
            .map(Self)
    }

    pub fn get(self) -> u8 {
        self.0
    }

    pub fn each(self) -> impl Iterator<Item = Hour> {
        (1..=self.0).map(Hour)
    }

    pub fn holds(self, hour: Hour) -> bool {
        hour.get() <= self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Weekdays(u8);

impl Weekdays {
    pub const CLAN_BATTLES: Weekdays = Weekdays(108);
    pub const EVERY_DAY: Weekdays = Weekdays(ALL_WEEKDAY_BITS);

    pub fn new(days: &[Weekday]) -> Option<Self> {
        let bits = days.iter().fold(0, |bits, day| bits | weekday_bit(*day));
        (bits != 0).then_some(Self(bits))
    }

    pub fn from_bits(bits: i64) -> Option<Self> {
        u8::try_from(bits)
            .ok()
            .filter(|bits| (1..=ALL_WEEKDAY_BITS).contains(bits))
            .map(Self)
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn contains(self, day: Weekday) -> bool {
        self.0 & weekday_bit(day) != 0
    }

    pub fn days(self) -> Vec<Weekday> {
        WEEK.into_iter().filter(|day| self.contains(*day)).collect()
    }
}

fn weekday_bit(day: Weekday) -> u8 {
    1 << day.to_monday_zero_offset()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StartTime {
    hour: i8,
    minute: i8,
}

impl StartTime {
    pub fn new(hour: i8, minute: i8) -> Option<Self> {
        ((0..=LAST_HOUR_OF_DAY).contains(&hour) && (0..=LAST_MINUTE_OF_HOUR).contains(&minute))
            .then_some(Self { hour, minute })
    }

    pub fn parse(text: &str) -> Option<Self> {
        let (hour, minute) = text.trim().split_once(':')?;
        let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
        if !(1..=2).contains(&hour.len()) || minute.len() != 2 || !digits(hour) || !digits(minute) {
            return None;
        }
        Self::new(hour.parse().ok()?, minute.parse().ok()?)
    }

    pub fn from_minutes(minutes: i64) -> Option<Self> {
        if !(0..MINUTES_PER_DAY).contains(&minutes) {
            return None;
        }
        Self::new(
            i8::try_from(minutes / MINUTES_PER_HOUR).ok()?,
            i8::try_from(minutes % MINUTES_PER_HOUR).ok()?,
        )
    }

    pub fn minutes(self) -> i64 {
        i64::from(self.hour) * MINUTES_PER_HOUR + i64::from(self.minute)
    }

    pub fn hour(self) -> i8 {
        self.hour
    }

    pub fn minute(self) -> i8 {
        self.minute
    }
}

impl fmt::Display for StartTime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:02}:{:02}", self.hour, self.minute)
    }
}

#[derive(Debug, Clone)]
pub struct Zone {
    name: String,
    time_zone: TimeZone,
}

impl PartialEq for Zone {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Zone {}

impl Zone {
    pub fn find(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > ZONE_NAME_LIMIT {
            return None;
        }
        if !text.contains('/') && !text.eq_ignore_ascii_case(UTC_NAME) {
            return None;
        }
        let time_zone = TimeZone::get(text).ok()?;
        let name = time_zone.iana_name()?.to_owned();
        Some(Self { name, time_zone })
    }

    pub fn utc() -> Self {
        Self {
            name: UTC_NAME.to_owned(),
            time_zone: TimeZone::UTC,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

pub fn zone_names() -> Vec<String> {
    let names: BTreeSet<String> = jiff::tz::db()
        .available()
        .filter_map(|name| Zone::find(name.as_str()))
        .map(|zone| zone.name)
        .collect();
    names.into_iter().collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    weekdays: Weekdays,
    start: StartTime,
    zone: Zone,
    hours: Hours,
}

impl Schedule {
    pub fn new(weekdays: Weekdays, start: StartTime, zone: Zone, hours: Hours) -> Self {
        Self {
            weekdays,
            start,
            zone,
            hours,
        }
    }

    pub fn clan_battles() -> Self {
        Self::new(
            Weekdays::CLAN_BATTLES,
            CLAN_BATTLES_START,
            Zone::utc(),
            CLAN_BATTLES_HOURS,
        )
    }

    pub fn rehearsal() -> Self {
        Self::new(
            Weekdays::EVERY_DAY,
            CLAN_BATTLES_START,
            Zone::utc(),
            CLAN_BATTLES_HOURS,
        )
    }

    pub fn weekdays(&self) -> Weekdays {
        self.weekdays
    }

    pub fn start(&self) -> StartTime {
        self.start
    }

    pub fn zone(&self) -> &Zone {
        &self.zone
    }

    pub fn hours(&self) -> Hours {
        self.hours
    }

    pub fn session_at(&self, date: Date) -> Option<Session> {
        let start = date
            .to_datetime(time(self.start.hour, self.start.minute, 0, 0))
            .to_zoned(self.zone.time_zone.clone())
            .ok()?;
        Some(Session {
            date,
            start_unix: start.timestamp().as_second(),
            hours: self.hours,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Session {
    date: Date,
    start_unix: i64,
    hours: Hours,
}

impl Session {
    pub fn date(self) -> Date {
        self.date
    }

    pub fn label(self) -> String {
        self.date.to_string()
    }

    pub fn start_unix(self) -> i64 {
        self.start_unix
    }

    pub fn hours(self) -> Hours {
        self.hours
    }

    pub fn hour_start_unix(self, hour: Hour) -> i64 {
        self.start_unix + i64::from(hour.get() - 1) * SECONDS_PER_HOUR
    }

    pub fn hour_end_unix(self, hour: Hour) -> i64 {
        self.hour_start_unix(hour) + SECONDS_PER_HOUR
    }

    pub fn end_unix(self) -> i64 {
        self.start_unix + i64::from(self.hours.get()) * SECONDS_PER_HOUR
    }

    pub fn post_at_unix(self) -> i64 {
        self.start_unix - POST_LEAD_SECONDS
    }

    pub fn close_unix(self) -> i64 {
        self.start_unix
    }

    pub fn remove_at_unix(self) -> i64 {
        self.end_unix() + REMOVE_AFTER_END_SECONDS
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    first_day: Date,
    last_day: Date,
    schedule: Schedule,
}

impl Span {
    pub fn new(first_day: Date, last_day: Date, schedule: Schedule) -> Option<Self> {
        (last_day >= first_day).then_some(Self {
            first_day,
            last_day,
            schedule,
        })
    }

    pub fn first_day(&self) -> Date {
        self.first_day
    }

    pub fn last_day(&self) -> Date {
        self.last_day
    }

    pub fn schedule(&self) -> &Schedule {
        &self.schedule
    }

    pub fn near(&self, today: Date) -> bool {
        within_months(self.first_day, self.last_day, today)
    }

    pub fn holds(&self, date: Date) -> bool {
        self.first_day <= date
            && date <= self.last_day
            && self.schedule.weekdays.contains(date.weekday())
    }

    pub fn session_on(&self, date: Date) -> Option<Session> {
        self.holds(date)
            .then(|| self.schedule.session_at(date))
            .flatten()
    }

    pub fn sessions(&self) -> impl Iterator<Item = Session> + '_ {
        self.first_day
            .series(1.day())
            .take_while(move |date| *date <= self.last_day)
            .filter(move |date| self.holds(*date))
            .filter_map(move |date| self.schedule.session_at(date))
    }

    pub fn session_count(&self) -> usize {
        self.sessions().count()
    }

    pub fn sessions_left(&self, now_unix: i64) -> usize {
        self.sessions()
            .filter(|session| session.start_unix() > now_unix)
            .count()
    }

    pub fn next_session(&self, now_unix: i64) -> Option<Session> {
        self.sessions()
            .find(|session| session.start_unix() > now_unix)
    }

    pub fn due_sessions(&self, now_unix: i64) -> impl Iterator<Item = Session> + '_ {
        self.sessions()
            .filter(move |session| session.start_unix() > now_unix)
            .filter(move |session| session.post_at_unix() <= now_unix)
    }

    pub fn last_moment_unix(&self) -> Option<i64> {
        self.sessions().last().map(Session::remove_at_unix)
    }
}

fn within_months(first_day: Date, last_day: Date, today: Date) -> bool {
    let Ok(earliest) = today.checked_sub(RANGE_MONTHS.months()) else {
        return false;
    };
    let Ok(latest) = today.checked_add(RANGE_MONTHS.months()) else {
        return false;
    };
    earliest <= first_day && last_day <= latest
}

pub fn rehearsal_span(today: Date, sessions: u32, now_unix: i64) -> Option<Span> {
    let sessions = sessions.clamp(1, REHEARSAL_SESSIONS_MAX);
    let schedule = Schedule::rehearsal();
    let tomorrow = today.checked_add(1.day()).ok()?;
    let first_day = if schedule
        .session_at(tomorrow)
        .is_some_and(|session| session.post_at_unix() <= now_unix)
    {
        tomorrow.checked_add(1.day()).ok()?
    } else {
        tomorrow
    };
    let last_day = first_day.checked_add(i64::from(sessions - 1).days()).ok()?;
    Span::new(first_day, last_day, schedule)
}

pub fn parse_day(text: &str) -> Option<Date> {
    if text.len() != DAY_TEXT_LENGTH {
        return None;
    }
    text.parse::<Date>().ok()
}

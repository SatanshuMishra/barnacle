use jiff::ToSpan;
use jiff::civil::Date;
use jiff::civil::Weekday;
use jiff::civil::time;
use jiff::tz::Offset;

pub const CB_WEEKDAYS: [Weekday; 4] = [
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Saturday,
    Weekday::Sunday,
];
pub const START_HOUR: i8 = 23;
pub const START_MINUTE: i8 = 30;
pub const HOURS_PER_NIGHT: u8 = 4;
pub const SECONDS_PER_HOUR: i64 = 3600;
pub const POST_LEAD_SECONDS: i64 = 24 * SECONDS_PER_HOUR;
pub const REMOVE_AFTER_END_SECONDS: i64 = 30 * 60;

pub const RANGE_MONTHS: i32 = 6;

const DAY_TEXT_LENGTH: usize = 10;

pub fn today(now_unix: i64) -> Option<Date> {
    let stamp = jiff::Timestamp::from_second(now_unix).ok()?;
    Some(Offset::UTC.to_datetime(stamp).date())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hour(u8);

impl Hour {
    pub const ALL: [Hour; 4] = [Hour(1), Hour(2), Hour(3), Hour(4)];

    pub fn new(value: u8) -> Option<Self> {
        (1..=HOURS_PER_NIGHT)
            .contains(&value)
            .then_some(Self(value))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Night {
    date: Date,
    start_unix: i64,
}

impl Night {
    pub fn new(date: Date) -> Option<Self> {
        if !CB_WEEKDAYS.contains(&date.weekday()) {
            return None;
        }
        let start_unix = Offset::UTC
            .to_timestamp(date.to_datetime(time(START_HOUR, START_MINUTE, 0, 0)))
            .ok()?
            .as_second();
        Some(Self { date, start_unix })
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::new(parse_day(text)?)
    }

    pub fn date(self) -> Date {
        self.date
    }

    pub fn label(self) -> String {
        self.date.to_string()
    }

    pub fn start_unix(self) -> i64 {
        self.start_unix
    }

    pub fn hour_start_unix(self, hour: Hour) -> i64 {
        self.start_unix + i64::from(hour.get() - 1) * SECONDS_PER_HOUR
    }

    pub fn hour_end_unix(self, hour: Hour) -> i64 {
        self.hour_start_unix(hour) + SECONDS_PER_HOUR
    }

    pub fn end_unix(self) -> i64 {
        self.start_unix + i64::from(HOURS_PER_NIGHT) * SECONDS_PER_HOUR
    }

    pub fn post_at_unix(self) -> i64 {
        self.start_unix - POST_LEAD_SECONDS
    }

    pub fn remove_at_unix(self) -> i64 {
        self.end_unix() + REMOVE_AFTER_END_SECONDS
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    first_day: Date,
    last_day: Date,
}

impl Range {
    pub fn new(first_day: Date, last_day: Date) -> Option<Self> {
        (last_day >= first_day).then_some(Self {
            first_day,
            last_day,
        })
    }

    pub fn near(self, today: Date) -> bool {
        let Ok(earliest) = today.checked_sub(RANGE_MONTHS.months()) else {
            return false;
        };
        let Ok(latest) = today.checked_add(RANGE_MONTHS.months()) else {
            return false;
        };
        earliest <= self.first_day && self.last_day <= latest
    }

    pub fn first_day(self) -> Date {
        self.first_day
    }

    pub fn last_day(self) -> Date {
        self.last_day
    }

    pub fn nights(self) -> impl Iterator<Item = Night> {
        self.first_day
            .series(1.day())
            .take_while(move |date| *date <= self.last_day)
            .filter_map(Night::new)
    }

    pub fn night_count(self) -> usize {
        self.nights().count()
    }

    pub fn nights_left(self, now_unix: i64) -> usize {
        self.nights()
            .filter(|night| night.start_unix() > now_unix)
            .count()
    }

    pub fn holds(self, night: Night) -> bool {
        self.first_day <= night.date() && night.date() <= self.last_day
    }

    pub fn next_night(self, now_unix: i64) -> Option<Night> {
        self.nights().find(|night| night.start_unix() > now_unix)
    }

    pub fn due_night(self, now_unix: i64) -> Option<Night> {
        self.next_night(now_unix)
            .filter(|night| night.post_at_unix() <= now_unix)
    }

    pub fn overlaps(self, other: Range) -> bool {
        self.first_day <= other.last_day && other.first_day <= self.last_day
    }

    pub fn last_moment_unix(self) -> Option<i64> {
        self.nights().last().map(Night::remove_at_unix)
    }
}

pub fn parse_day(text: &str) -> Option<Date> {
    if text.len() != DAY_TEXT_LENGTH {
        return None;
    }
    text.parse::<Date>().ok()
}

#![allow(dead_code)]

pub mod fakes;
pub mod logs;

use barnacle_bot::attendance::MenuOutcome;
use barnacle_bot::attendance::Signups;
use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::attendance_store::CreateOutcome;
use barnacle_bot::attendance_store::NewEvent;
use barnacle_bot::discord;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::Ping;
use barnacle_bot::ids::RoleId;
use barnacle_bot::schedule::Span;
use barnacle_bot::solves::Solves;
use barnacle_bot::text::Refusal;
use barnacle_catalog::Catalog;
use barnacle_catalog::Nation;
use barnacle_catalog::ParamId;
use barnacle_catalog::Provenance;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipGroup;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::ShipName;
use barnacle_catalog::Silhouette;
use barnacle_catalog::Tier;
use barnacle_catalog::curation::CurationConfig;
use barnacle_guess::RoundOptions;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use fakes::FakeBoard;
use jiff::ToSpan;
use jiff::civil::Date;
use sqlx::Row;
use sqlx::sqlite::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

pub const MIGRATION: &str = include_str!("../../../../migrations/0001_guess_solves.sql");
pub const CB_MIGRATION: &str = include_str!("../../../../migrations/0002_cb_attendance.sql");
pub const CB_CONTROLS: &str = include_str!("../../../../migrations/0003_cb_season_controls.sql");
pub const CB_HARNESS: &str = include_str!("../../../../migrations/0004_cb_rehearsal_harness.sql");
pub const CB_PING_ROLES: &str = include_str!("../../../../migrations/0006_cb_ping_roles.sql");
pub const CB_PING_ROLES_DOWN: &str =
    include_str!("../../../../migrations/0006_cb_ping_roles.down.sql");
pub const CB_EVENTS: &str = include_str!("../../../../migrations/0008_events.sql");
pub const CB_EVENTS_DOWN: &str = include_str!("../../../../migrations/0008_events.down.sql");
const CLI_DIRECTIVE: &str = ".bail on\n";
pub const BUILD: u32 = 13187581;
pub const FIRST_ZONES: [&str; 25] = [
    "Africa/Abidjan",
    "Africa/Accra",
    "Africa/Addis_Ababa",
    "Africa/Algiers",
    "Africa/Asmara",
    "Africa/Asmera",
    "Africa/Bamako",
    "Africa/Bangui",
    "Africa/Banjul",
    "Africa/Bissau",
    "Africa/Blantyre",
    "Africa/Brazzaville",
    "Africa/Bujumbura",
    "Africa/Cairo",
    "Africa/Casablanca",
    "Africa/Ceuta",
    "Africa/Conakry",
    "Africa/Dakar",
    "Africa/Dar_es_Salaam",
    "Africa/Djibouti",
    "Africa/Douala",
    "Africa/El_Aaiun",
    "Africa/Freetown",
    "Africa/Gaborone",
    "Africa/Harare",
];

pub type StoredPost = (i64, String, i64, String, i64);
pub type StoredAnswer = (i64, String, i64, i64, i64, i64, i64);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEventRow {
    pub id: i64,
    pub guild_id: i64,
    pub channel_id: i64,
    pub name: String,
    pub first_day: String,
    pub last_day: String,
    pub weekdays: i64,
    pub start_minute: i64,
    pub time_zone: String,
    pub hours: i64,
    pub created_by: i64,
    pub created_at_ms: i64,
    pub ended_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    pub row: Option<StoredEventRow>,
    pub ping_roles: Vec<(i64, i64)>,
    pub posts: Vec<StoredPost>,
    pub answers: Vec<StoredAnswer>,
}

pub fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

pub fn tier(value: u32) -> Tier {
    Tier::new(value).unwrap()
}

pub fn at(millis: u64) -> Snowflake {
    Snowflake::new(millis << 22)
}

pub fn ship(value: &str, name: &str, tier_value: u32, silhouette: &str) -> Ship {
    Ship {
        id: ParamId::new(1),
        index: index(value),
        tier: tier(tier_value),
        group: ShipGroup::new("upgradeable"),
        class: ShipClass::Battleship,
        nation: Nation::new("Japan"),
        is_paper: false,
        name: Some(ShipName {
            short: name.to_owned(),
            full: None,
        }),
        silhouette: Some(Silhouette {
            sha256: silhouette.to_owned(),
        }),
        hull_model: None,
    }
}

pub fn fleet(size: usize) -> Vec<Ship> {
    (0..size)
        .map(|number| {
            ship(
                &format!("PXSX{number:03}"),
                &format!("Hull {number}"),
                8,
                &format!("silhouette-{number}"),
            )
        })
        .collect()
}

pub fn catalog(ships: Vec<Ship>) -> Catalog {
    Catalog {
        provenance: Provenance {
            game_version: "15.8.0".to_owned(),
            build: BUILD,
            data_repo_commit: "442496ea2f27517507a562f6eb3ceee06003d3da".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        ships,
    }
}

pub fn curation_text(tables: &str) -> String {
    format!("reviewed_through = {BUILD}\ngroups = [\"upgradeable\"]\n{tables}")
}

pub fn curation(tables: &str) -> CurationConfig {
    CurationConfig::from_toml(&curation_text(tables)).unwrap()
}

pub async fn memory_pool() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap()
}

pub async fn migrated_pool() -> SqlitePool {
    let pool = memory_pool().await;
    sqlx::raw_sql(MIGRATION).execute(&pool).await.unwrap();
    pool
}

pub async fn solves() -> Solves {
    Solves::with_pool(migrated_pool().await).await.unwrap()
}

pub async fn run_script(pool: &SqlitePool, script: &'static str) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(script.trim_start_matches(CLI_DIRECTIVE))
        .execute(pool)
        .await
        .map(|_| ())
}

pub async fn seasons_pool() -> SqlitePool {
    let pool = memory_pool().await;
    sqlx::raw_sql(CB_MIGRATION).execute(&pool).await.unwrap();
    sqlx::raw_sql(CB_CONTROLS.trim_start_matches(CLI_DIRECTIVE))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(CB_HARNESS.trim_start_matches(CLI_DIRECTIVE))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(CB_PING_ROLES.trim_start_matches(CLI_DIRECTIVE))
        .execute(&pool)
        .await
        .unwrap();
    pool
}

pub async fn attendance_pool() -> SqlitePool {
    let pool = seasons_pool().await;
    sqlx::raw_sql(CB_EVENTS.trim_start_matches(CLI_DIRECTIVE))
        .execute(&pool)
        .await
        .unwrap();
    pool
}

pub async fn attendance() -> Attendance {
    Attendance::with_pool(attendance_pool().await)
        .await
        .unwrap()
}

pub fn every_refusal() -> Vec<Refusal> {
    let refusals = vec![
        Refusal::WrongChannelType,
        Refusal::MissingBotPermissions {
            names: vec!["Send Messages"],
            channel: Some(ChannelId::new(10)),
        },
        Refusal::DateFormat,
        Refusal::LastDayBeforeFirst,
        Refusal::DatesTooFar,
        Refusal::NoSessionsLeft,
        Refusal::NameTaken("Ranked Sprint".to_owned()),
        Refusal::NameFormat,
        Refusal::EventNotFound,
        Refusal::NothingToChange,
        Refusal::RehearsalOnly,
        Refusal::RehearsalRunning,
        Refusal::NothingPending,
        Refusal::RoundAlreadyRunning,
        Refusal::EmptyPool(RoundOptions::default()),
        Refusal::NoShipMatches,
        Refusal::NameEmpty,
        Refusal::NameTooLong { limit: 100 },
        Refusal::NotAHub,
        Refusal::CategoryAndTopLevel,
        Refusal::HubNothingToChange,
        Refusal::PingBlockedByChannel {
            pings: vec![Ping::Role(RoleId::new(4))],
            channel: ChannelId::new(10),
        },
        Refusal::PingMixesEveryone,
        Refusal::NothingToRepost {
            name: "Ranked Sprint".to_owned(),
            next_post_at: Some(1_790_811_000),
            sessions_left: 3,
        },
        Refusal::RepostNothingToPing,
        Refusal::RepostPingNotAllowed,
        Refusal::RepostPingsChanged,
        Refusal::RepostSuperseded,
        Refusal::SeriesRoundsOutOfRange,
        Refusal::TemplateUnknown,
        Refusal::MissingOptions(vec!["name", "first_day"]),
        Refusal::StartFormat,
        Refusal::HoursOutOfRange,
        Refusal::ZoneUnknown,
        Refusal::WeekdaysInvalid,
        Refusal::TooManyRoles,
        Refusal::PanelUnknown,
        Refusal::PanelNotYours,
        Refusal::PanelExpired,
        Refusal::ChannelUnknown,
    ];
    for refusal in &refusals {
        match refusal {
            Refusal::WrongChannelType
            | Refusal::MissingBotPermissions { .. }
            | Refusal::DateFormat
            | Refusal::LastDayBeforeFirst
            | Refusal::DatesTooFar
            | Refusal::NoSessionsLeft
            | Refusal::NameTaken(_)
            | Refusal::NameFormat
            | Refusal::EventNotFound
            | Refusal::NothingToChange
            | Refusal::RehearsalOnly
            | Refusal::RehearsalRunning
            | Refusal::NothingPending
            | Refusal::RoundAlreadyRunning
            | Refusal::EmptyPool(_)
            | Refusal::NoShipMatches
            | Refusal::NameEmpty
            | Refusal::NameTooLong { .. }
            | Refusal::NotAHub
            | Refusal::CategoryAndTopLevel
            | Refusal::HubNothingToChange
            | Refusal::PingBlockedByChannel { .. }
            | Refusal::PingMixesEveryone
            | Refusal::NothingToRepost { .. }
            | Refusal::RepostNothingToPing
            | Refusal::RepostPingNotAllowed
            | Refusal::RepostPingsChanged
            | Refusal::RepostSuperseded
            | Refusal::SeriesRoundsOutOfRange
            | Refusal::TemplateUnknown
            | Refusal::MissingOptions(_)
            | Refusal::StartFormat
            | Refusal::HoursOutOfRange
            | Refusal::ZoneUnknown
            | Refusal::WeekdaysInvalid
            | Refusal::TooManyRoles
            | Refusal::PanelUnknown
            | Refusal::PanelNotYours
            | Refusal::PanelExpired
            | Refusal::ChannelUnknown => {}
        }
    }
    refusals
}

pub fn date_run(first: &str) -> Vec<(String, String)> {
    first
        .parse::<Date>()
        .unwrap()
        .series(1.day())
        .take(25)
        .map(|date| (date.strftime("%Y-%m-%d (%A)").to_string(), date.to_string()))
        .collect()
}

pub async fn posts_and_answers(
    pool: &SqlitePool,
    event: i64,
) -> (Vec<StoredPost>, Vec<StoredAnswer>) {
    let posts = sqlx::query_as(
        "SELECT season_id, night, message_id, state, posted_at_ms FROM cb_posts WHERE season_id = ? ORDER BY night",
    )
    .bind(event)
    .fetch_all(pool)
    .await
    .unwrap();
    let answers = sqlx::query_as(
        "SELECT season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms FROM cb_marks WHERE season_id = ? ORDER BY night, user_id, hour",
    )
    .bind(event)
    .fetch_all(pool)
    .await
    .unwrap();
    (posts, answers)
}

pub async fn every_answer(pool: &SqlitePool) -> Vec<StoredAnswer> {
    sqlx::query_as(
        "SELECT season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms FROM cb_marks ORDER BY season_id, night, user_id, hour",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

pub async fn event_state(pool: &SqlitePool, event: i64) -> StoredEvent {
    let row = sqlx::query(
        "SELECT id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms, ended_at_ms FROM cb_seasons WHERE id = ?",
    )
    .bind(event)
    .fetch_optional(pool)
    .await
    .unwrap()
    .map(|row| StoredEventRow {
        id: row.get("id"),
        guild_id: row.get("guild_id"),
        channel_id: row.get("channel_id"),
        name: row.get("name"),
        first_day: row.get("first_day"),
        last_day: row.get("last_day"),
        weekdays: row.get("weekdays"),
        start_minute: row.get("start_minute"),
        time_zone: row.get("time_zone"),
        hours: row.get("hours"),
        created_by: row.get("created_by"),
        created_at_ms: row.get("created_at_ms"),
        ended_at_ms: row.get("ended_at_ms"),
    });
    let ping_roles = sqlx::query_as(
        "SELECT position, role_id FROM cb_season_ping_roles WHERE season_id = ? ORDER BY position",
    )
    .bind(event)
    .fetch_all(pool)
    .await
    .unwrap();
    let (posts, answers) = posts_and_answers(pool, event).await;
    StoredEvent {
        row,
        ping_roles,
        posts,
        answers,
    }
}

pub async fn hours_menu_json(
    span: Span,
    answers: &[(u64, u8, bool)],
    member: u64,
    now_unix: i64,
) -> serde_json::Value {
    let pool = attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    let new = NewEvent {
        guild: GuildId::new(1),
        channel: ChannelId::new(10),
        name: "Ranked Sprint".to_owned(),
        span,
        created_by: UserId::new(100),
        created_at_ms: 0,
        ping_roles: Vec::new(),
    };
    let event = match store.create_event(&new).await.unwrap() {
        CreateOutcome::Created(event) => event,
        CreateOutcome::NameTaken => panic!("the event was not stored"),
    };
    let date = event.span.first_day();
    store
        .record_post(event.id, date, Snowflake::new(7), 0)
        .await
        .unwrap();
    for (user, hour, attending) in answers {
        sqlx::query(
            "INSERT INTO cb_marks (season_id, night, user_id, hour, attending, answered_at_ms, changed_at_ms) VALUES (?, ?, ?, ?, ?, 0, 0)",
        )
        .bind(event.id)
        .bind(date.to_string())
        .bind(i64::try_from(*user).unwrap())
        .bind(i64::from(*hour))
        .bind(i64::from(*attending))
        .execute(&pool)
        .await
        .unwrap();
    }
    let signups = Signups::new(FakeBoard::new(), store);
    let now_ms = u64::try_from(now_unix).unwrap() * 1_000;
    let MenuOutcome::Open { view, attending } = signups
        .hours_menu(
            GuildId::new(1),
            event.id,
            date,
            UserId::new(member),
            now_unix,
            now_ms,
        )
        .await
    else {
        panic!("the hours menu did not open");
    };
    serde_json::to_value(discord::hours_menu_reply(&view, &attending)).unwrap()
}

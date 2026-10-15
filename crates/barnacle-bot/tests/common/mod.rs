#![allow(dead_code)]

pub mod fakes;
pub mod logs;

use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::Ping;
use barnacle_bot::ids::RoleId;
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

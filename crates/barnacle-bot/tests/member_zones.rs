mod common;

use std::path::Path;

use barnacle_bot::schedule;
use barnacle_bot::schedule::ZoneProblem;
use barnacle_bot::zone_store::ZoneStore;
use barnacle_guess::UserId;
use common::attendance_pool;
use sqlx::sqlite::SqlitePool;

const ZONE_MIGRATION: &str = "0011_member_zones.sql";
const CLI_DIRECTIVE: &str = ".bail on\n";

async fn zone_pool() -> SqlitePool {
    let pool = attendance_pool().await;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../migrations")
        .join(ZONE_MIGRATION);
    let script = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
    sqlx::raw_sql(sqlx::AssertSqlSafe(
        script.trim_start_matches(CLI_DIRECTIVE).to_owned(),
    ))
    .execute(&pool)
    .await
    .unwrap();
    pool
}

fn holds(suggestions: &[String], name: &str) -> bool {
    suggestions.iter().any(|suggestion| suggestion == name)
}

#[test]
fn typing_part_of_a_city_finds_its_zone() {
    let edmonton = schedule::zone_suggestions("edm");
    assert!(holds(&edmonton, "America/Edmonton"), "{edmonton:?}");
    let new_york = schedule::zone_suggestions("new york");
    assert!(holds(&new_york, "America/New_York"), "{new_york:?}");
    let berlin = schedule::zone_suggestions("BERLIN");
    assert!(holds(&berlin, "Europe/Berlin"), "{berlin:?}");
}

#[test]
fn suggestions_stop_at_25() {
    assert_eq!(schedule::zone_suggestions("a").len(), 25);
}

#[test]
fn an_empty_search_offers_the_common_zones() {
    assert_eq!(
        schedule::zone_suggestions(""),
        [
            "America/Vancouver",
            "America/Edmonton",
            "America/Denver",
            "America/Chicago",
            "America/Toronto",
            "America/New_York",
            "Europe/London",
            "Europe/Berlin",
            "UTC",
        ]
    );
}

#[test]
fn every_zone_can_be_found_by_its_name() {
    let names = schedule::zone_names();
    assert!(!names.is_empty());
    for name in names {
        let found = schedule::zone_suggestions(&name);
        assert!(holds(&found, &name), "{name} is not in {found:?}");
    }
}

#[test]
fn nonsense_finds_nothing() {
    assert_eq!(schedule::zone_suggestions("zzqx"), Vec::<String>::new());
}

#[test]
fn a_given_zone_wins_over_the_remembered_one() {
    let resolved = schedule::zone_for(Some("Europe/Berlin"), Some("America/Edmonton"));
    assert_eq!(
        resolved.map(|zone| zone.name().to_owned()),
        Ok("Europe/Berlin".to_owned())
    );
}

#[test]
fn the_remembered_zone_fills_in() {
    let resolved = schedule::zone_for(None, Some("America/Edmonton"));
    assert_eq!(
        resolved.map(|zone| zone.name().to_owned()),
        Ok("America/Edmonton".to_owned())
    );
}

#[test]
fn no_known_zone_is_missing() {
    assert_eq!(
        schedule::zone_for(None, None).err(),
        Some(ZoneProblem::Missing)
    );
    assert_eq!(
        schedule::zone_for(None, Some("Mars/Olympus")).err(),
        Some(ZoneProblem::Missing)
    );
}

#[test]
fn an_unknown_given_zone_is_named() {
    assert_eq!(
        schedule::zone_for(Some("Mars/Olympus"), Some("America/Edmonton")).err(),
        Some(ZoneProblem::Unknown("Mars/Olympus".to_owned()))
    );
}

#[tokio::test]
async fn a_members_zone_is_remembered() {
    let store = ZoneStore::with_pool(zone_pool().await).await.unwrap();
    store
        .remember(UserId::new(100), "America/Edmonton")
        .await
        .unwrap();
    store
        .remember(UserId::new(100), "Europe/Berlin")
        .await
        .unwrap();
    assert_eq!(
        store.remembered(UserId::new(100)).await.unwrap(),
        Some("Europe/Berlin".to_owned())
    );
    assert_eq!(store.remembered(UserId::new(200)).await.unwrap(), None);
}

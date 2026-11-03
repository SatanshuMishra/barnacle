mod common;

use std::path::Path;

use barnacle_bot::access;
use barnacle_bot::access_store::AccessStore;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::RoleId;
use common::attendance_pool;
use sqlx::sqlite::SqlitePool;

const ACCESS_MIGRATION: &str = "0010_organiser_roles.sql";
const CLI_DIRECTIVE: &str = ".bail on\n";

async fn access_pool() -> SqlitePool {
    let pool = attendance_pool().await;
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../migrations")
        .join(ACCESS_MIGRATION);
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

async fn store() -> AccessStore {
    AccessStore::with_pool(access_pool().await).await.unwrap()
}

#[test]
fn an_administrator_may_organise_with_no_roles_set() {
    assert!(access::may_organise(true, &[], &[]));
}

#[test]
fn a_member_holding_an_organiser_role_may_organise() {
    assert!(access::may_organise(
        false,
        &[RoleId::new(5), RoleId::new(9)],
        &[RoleId::new(9)]
    ));
}

#[test]
fn a_member_without_an_organiser_role_may_not_organise() {
    assert!(!access::may_organise(
        false,
        &[RoleId::new(5)],
        &[RoleId::new(9)]
    ));
    assert!(!access::may_organise(false, &[RoleId::new(5)], &[]));
}

#[tokio::test]
async fn organiser_roles_are_stored_per_server() {
    let store = store().await;
    store
        .set_organiser_roles(GuildId::new(1), &[RoleId::new(9), RoleId::new(7)])
        .await
        .unwrap();
    assert_eq!(
        store.organiser_roles(GuildId::new(1)).await.unwrap(),
        vec![RoleId::new(7), RoleId::new(9)]
    );
    assert_eq!(
        store.organiser_roles(GuildId::new(2)).await.unwrap(),
        Vec::<RoleId>::new()
    );
}

#[tokio::test]
async fn saving_no_roles_clears_the_set() {
    let store = store().await;
    store
        .set_organiser_roles(GuildId::new(1), &[RoleId::new(9)])
        .await
        .unwrap();
    store
        .set_organiser_roles(GuildId::new(1), &[])
        .await
        .unwrap();
    assert_eq!(
        store.organiser_roles(GuildId::new(1)).await.unwrap(),
        Vec::<RoleId>::new()
    );
}

mod common;

use std::path::Path;

use barnacle_bot::config::CommandScope;
use barnacle_bot::config::Config;
use barnacle_bot::startup;
use barnacle_bot::startup::StartupError;
use barnacle_catalog::Catalog;
use common::catalog;
use common::curation_text;
use common::index;
use common::ship;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;

const NAME: &str = "15.8.0_13187581_r4";

struct Layout {
    _dir: tempfile::TempDir,
    config: Config,
}

fn layout(catalog: &Catalog, silhouettes: &[&str], curation: &str) -> Layout {
    let dir = tempfile::tempdir().unwrap();
    let catalogs = dir.path().join("data").join("catalog");
    let silhouette_dir = catalogs.join(NAME).join("silhouettes");
    std::fs::create_dir_all(&silhouette_dir).unwrap();
    std::fs::write(
        catalogs.join(NAME).join("catalog.json"),
        catalog.to_json().unwrap(),
    )
    .unwrap();
    std::fs::write(catalogs.join("current"), format!("{NAME}\n")).unwrap();
    for ship in silhouettes {
        std::fs::write(silhouette_dir.join(format!("{ship}.png")), b"png").unwrap();
    }
    let curation_path = dir.path().join("ships.toml");
    std::fs::write(&curation_path, curation).unwrap();
    let config = Config {
        data_dir: dir.path().join("data"),
        curation: curation_path,
        database: dir.path().join("barnacle.sqlite3"),
        commands: CommandScope::Global,
        rehearsal: Vec::new(),
    };
    Layout { _dir: dir, config }
}

fn yamato_catalog() -> Catalog {
    catalog(vec![
        ship("PJSB018", "Yamato", 10, "yamato"),
        ship("PJSB019", "Musashi", 9, "musashi"),
    ])
}

#[test]
fn a_complete_layout_loads() {
    let layout = layout(
        &yamato_catalog(),
        &["PJSB018", "PJSB019"],
        &curation_text(""),
    );
    let loaded = startup::load(&layout.config).unwrap();
    assert_eq!(loaded.catalog_name, NAME);
    assert_eq!(loaded.catalog, yamato_catalog());
    assert_eq!(loaded.directory.resolve("musashi"), Some(&index("PJSB019")));
    assert_eq!(
        loaded
            .book
            .pool(&barnacle_guess::RoundOptions::default())
            .len(),
        2
    );
}

#[test]
fn no_selected_catalog_is_refused() {
    let layout = layout(
        &yamato_catalog(),
        &["PJSB018", "PJSB019"],
        &curation_text(""),
    );
    std::fs::remove_file(layout.config.catalogs().join("current")).unwrap();
    assert!(matches!(
        startup::load(&layout.config),
        Err(StartupError::NoCurrentCatalog { .. })
    ));
}

#[test]
fn curation_problems_are_refused_and_listed() {
    let layout = layout(
        &yamato_catalog(),
        &["PJSB018", "PJSB019"],
        "reviewed_through = 1\ngroups = [\"upgradeable\"]\n",
    );
    let error = startup::load(&layout.config).err().unwrap();
    assert!(matches!(error, StartupError::Curation { .. }));
    assert_eq!(
        startup::describe(&error),
        "curation has 1 problem(s); run `barnacle-data validate`\n  - curation was reviewed through build 1, but the catalog is build 13187581"
    );
}

#[test]
fn a_missing_silhouette_is_refused_and_named() {
    let layout = layout(&yamato_catalog(), &["PJSB018"], &curation_text(""));
    let error = startup::load(&layout.config).err().unwrap();
    assert!(matches!(
        &error,
        StartupError::MissingSilhouettes { missing, .. } if missing == &[index("PJSB019")]
    ));
    assert!(startup::describe(&error).ends_with("\n  - PJSB019"));
}

#[test]
fn a_missing_or_blank_token_is_refused() {
    assert!(matches!(
        startup::read_token(None),
        Err(StartupError::MissingToken)
    ));
    assert!(matches!(
        startup::read_token(Some("  ".to_owned())),
        Err(StartupError::MissingToken)
    ));
    assert_eq!(
        startup::read_token(Some(" secret-token\n".to_owned())).unwrap(),
        "secret-token"
    );
}

#[test]
fn a_missing_config_file_is_refused() {
    assert!(matches!(
        startup::read_config(Path::new("/nonexistent/barnacle.toml")),
        Err(StartupError::ConfigIo { .. })
    ));
}

#[tokio::test]
async fn a_missing_database_explains_how_to_create_it() {
    let layout = layout(
        &yamato_catalog(),
        &["PJSB018", "PJSB019"],
        &curation_text(""),
    );
    let error = startup::open_solves(&layout.config.database)
        .await
        .err()
        .unwrap();
    assert!(matches!(error, StartupError::Database { .. }));
    assert!(error.to_string().contains(&format!(
        "sqlite3 {} < migrations/0001_guess_solves.sql",
        layout.config.database.display()
    )));
}

#[tokio::test]
async fn a_missing_attendance_table_is_named_with_its_migration() {
    let layout = layout(
        &yamato_catalog(),
        &["PJSB018", "PJSB019"],
        &curation_text(""),
    );
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&layout.config.database)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::raw_sql(common::MIGRATION)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let error = startup::open_stores(&layout.config.database)
        .await
        .err()
        .unwrap();
    assert!(matches!(error, StartupError::Attendance { .. }));
    assert!(error.to_string().contains(&format!(
        "sqlite3 {} < migrations/0002_cb_attendance.sql",
        layout.config.database.display()
    )));
}

#[tokio::test]
async fn a_database_without_the_season_controls_names_the_third_migration() {
    let layout = layout(
        &yamato_catalog(),
        &["PJSB018", "PJSB019"],
        &curation_text(""),
    );
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&layout.config.database)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::raw_sql(common::MIGRATION)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(common::CB_MIGRATION)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let error = startup::open_stores(&layout.config.database)
        .await
        .err()
        .unwrap();
    assert!(matches!(error, StartupError::SeasonControls { .. }));
    assert!(error.to_string().contains(&format!(
        "sqlite3 {} < migrations/0003_cb_season_controls.sql",
        layout.config.database.display()
    )));
}

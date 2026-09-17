mod common;

use std::time::Duration;

use barnacle_bot::ids::GuildId;
use barnacle_bot::solves::Profile;
use barnacle_bot::solves::Ranking;
use barnacle_bot::solves::SolveRecord;
use barnacle_bot::solves::SolveStore;
use barnacle_bot::solves::Solves;
use barnacle_bot::solves::SolvesError;
use barnacle_bot::solves::Standing;
use barnacle_guess::UserId;
use common::index;
use common::memory_pool;
use common::migrated_pool;
use common::solves;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;

const GUILD: GuildId = GuildId::new(1);
const PLAYER: UserId = UserId::new(200);

fn win(guild: GuildId, user: UserId, millis: u64) -> SolveRecord {
    SolveRecord {
        guild,
        user,
        ship: index("PJSB018"),
        elapsed: Duration::from_millis(millis),
        solved_at_ms: 1_700_000_000_000,
    }
}

#[tokio::test]
async fn a_recorded_win_shows_in_the_profile() {
    let solves = solves().await;
    assert_eq!(
        solves.profile(GUILD, PLAYER).await.unwrap(),
        Profile {
            wins: 0,
            best: None
        }
    );
    solves.record(&win(GUILD, PLAYER, 3_251)).await.unwrap();
    solves.record(&win(GUILD, PLAYER, 4_000)).await.unwrap();
    assert_eq!(
        solves.profile(GUILD, PLAYER).await.unwrap(),
        Profile {
            wins: 2,
            best: Some(Duration::from_millis(3_251))
        }
    );
}

#[tokio::test]
async fn profiles_are_kept_per_server_and_per_player() {
    let solves = solves().await;
    solves.record(&win(GUILD, PLAYER, 3_000)).await.unwrap();
    assert_eq!(
        solves.profile(GuildId::new(2), PLAYER).await.unwrap().wins,
        0
    );
    assert_eq!(
        solves.profile(GUILD, UserId::new(201)).await.unwrap().wins,
        0
    );
}

#[tokio::test]
async fn only_a_strictly_faster_time_is_a_personal_best() {
    let solves = solves().await;
    assert!(solves.record(&win(GUILD, PLAYER, 5_000)).await.unwrap());
    assert!(!solves.record(&win(GUILD, PLAYER, 6_000)).await.unwrap());
    assert!(!solves.record(&win(GUILD, PLAYER, 5_000)).await.unwrap());
    assert!(solves.record(&win(GUILD, PLAYER, 4_999)).await.unwrap());
    assert!(
        solves
            .record(&win(GuildId::new(2), PLAYER, 9_000))
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn a_database_without_the_table_is_refused() {
    assert!(matches!(
        Solves::with_pool(memory_pool().await).await,
        Err(SolvesError::MissingTable)
    ));
}

#[tokio::test]
async fn a_table_with_other_columns_is_refused() {
    let pool = memory_pool().await;
    sqlx::raw_sql("CREATE TABLE guess_solves (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        Solves::with_pool(pool).await,
        Err(SolvesError::UnexpectedColumns { .. })
    ));
}

#[tokio::test]
async fn a_table_with_the_right_names_but_other_types_is_refused() {
    let pool = memory_pool().await;
    sqlx::raw_sql(
        "CREATE TABLE guess_solves (id INTEGER PRIMARY KEY, guild_id INTEGER NOT NULL, user_id INTEGER NOT NULL, ship_index TEXT NOT NULL, elapsed_ms TEXT NOT NULL, solved_at_ms INTEGER NOT NULL) STRICT",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        Solves::with_pool(pool).await,
        Err(SolvesError::UnexpectedColumns { .. })
    ));
}

#[tokio::test]
async fn an_id_too_large_for_sqlite_is_refused() {
    let solves = solves().await;
    assert!(matches!(
        solves
            .record(&win(GUILD, UserId::new(u64::MAX), 1_000))
            .await,
        Err(SolvesError::OutOfRange { .. })
    ));
}

#[tokio::test]
async fn opening_a_missing_file_fails_without_creating_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("barnacle.sqlite3");
    assert!(Solves::open(&path).await.is_err());
    assert!(!path.exists());
}

#[tokio::test]
async fn opening_a_migrated_file_works() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("barnacle.sqlite3");
    let setup = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::raw_sql(common::MIGRATION)
        .execute(&setup)
        .await
        .unwrap();
    setup.close().await;
    let solves = Solves::open(&path).await.unwrap();
    assert!(solves.record(&win(GUILD, PLAYER, 1_000)).await.unwrap());
}

fn standing(user: u64, wins: u64, best_millis: u64) -> Standing {
    Standing {
        user: UserId::new(user),
        wins,
        best: Duration::from_millis(best_millis),
    }
}

async fn contested_server() -> Solves {
    let solves = solves().await;
    let wins = [
        (207, 2_000),
        (201, 5_000),
        (201, 6_000),
        (201, 7_000),
        (202, 4_000),
        (202, 8_000),
        (202, 9_000),
        (203, 2_000),
        (204, 3_000),
        (204, 3_500),
        (205, 2_000),
        (205, 9_500),
    ];
    for (user, millis) in wins {
        solves
            .record(&win(GUILD, UserId::new(user), millis))
            .await
            .unwrap();
    }
    for _ in 0..4 {
        solves
            .record(&win(GuildId::new(2), UserId::new(206), 1_000))
            .await
            .unwrap();
    }
    solves
}

#[tokio::test]
async fn most_wins_ranks_by_wins_then_best_time_then_player() {
    let solves = contested_server().await;
    assert_eq!(
        solves
            .leaderboard(GUILD, Ranking::MostWins, 10)
            .await
            .unwrap(),
        [
            standing(202, 3, 4_000),
            standing(201, 3, 5_000),
            standing(205, 2, 2_000),
            standing(204, 2, 3_000),
            standing(203, 1, 2_000),
            standing(207, 1, 2_000),
        ]
    );
}

#[tokio::test]
async fn fastest_time_ranks_by_best_time_then_wins_then_player() {
    let solves = contested_server().await;
    assert_eq!(
        solves
            .leaderboard(GUILD, Ranking::FastestTime, 10)
            .await
            .unwrap(),
        [
            standing(205, 2, 2_000),
            standing(203, 1, 2_000),
            standing(207, 1, 2_000),
            standing(204, 2, 3_000),
            standing(202, 3, 4_000),
            standing(201, 3, 5_000),
        ]
    );
}

#[tokio::test]
async fn a_leaderboard_stops_at_its_size_and_counts_only_its_server() {
    let solves = contested_server().await;
    assert_eq!(
        solves
            .leaderboard(GUILD, Ranking::FastestTime, 2)
            .await
            .unwrap(),
        [standing(205, 2, 2_000), standing(203, 1, 2_000)]
    );
    assert_eq!(
        solves
            .leaderboard(GuildId::new(2), Ranking::MostWins, 10)
            .await
            .unwrap(),
        [standing(206, 4, 1_000)]
    );
    assert!(
        solves
            .leaderboard(GuildId::new(3), Ranking::MostWins, 10)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_negative_stored_player_id_is_reported() {
    let pool = migrated_pool().await;
    sqlx::raw_sql(
        "INSERT INTO guess_solves (guild_id, user_id, ship_index, elapsed_ms, solved_at_ms) VALUES (1, -5, 'PJSB018', 1000, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let solves = Solves::with_pool(pool).await.unwrap();
    assert!(matches!(
        solves.leaderboard(GUILD, Ranking::MostWins, 10).await,
        Err(SolvesError::Negative { value: -5 })
    ));
}

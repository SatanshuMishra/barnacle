mod common;

use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::attendance_store::AttendanceError;
use sqlx::sqlite::SqlitePool;

async fn at_0008_without_0014() -> SqlitePool {
    let pool = common::seasons_pool().await;
    common::run_script(&pool, common::CB_EVENTS).await.unwrap();
    pool
}

#[tokio::test]
async fn a_store_missing_the_0008_name_index_and_0014_names_the_index() {
    let pool = at_0008_without_0014().await;
    sqlx::raw_sql("DROP INDEX cb_seasons_live_name;")
        .execute(&pool)
        .await
        .unwrap();
    let error = Attendance::with_pool(pool).await.err().unwrap();
    assert!(
        matches!(
            error,
            AttendanceError::MissingIndex {
                index: "cb_seasons_live_name"
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_store_missing_the_rehearsal_clock_and_0014_names_the_clock() {
    let pool = at_0008_without_0014().await;
    sqlx::raw_sql("DROP TABLE cb_rehearsal_clock;")
        .execute(&pool)
        .await
        .unwrap();
    let error = Attendance::with_pool(pool).await.err().unwrap();
    assert!(
        matches!(
            error,
            AttendanceError::MissingTable {
                table: "cb_rehearsal_clock"
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_store_missing_only_0014_asks_for_it() {
    let pool = at_0008_without_0014().await;
    let error = Attendance::with_pool(pool).await.err().unwrap();
    assert!(
        matches!(error, AttendanceError::BeforePostTiming),
        "{error:?}"
    );
}

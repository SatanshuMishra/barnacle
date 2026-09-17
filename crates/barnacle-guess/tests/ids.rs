use std::time::Duration;

use barnacle_guess::Snowflake;
use barnacle_guess::UserId;

fn at(millis: u64, increment: u64) -> Snowflake {
    Snowflake::new((millis << 22) | increment)
}

#[test]
fn reads_the_timestamp_of_discords_documented_example_id() {
    assert_eq!(
        Snowflake::new(175_928_847_299_117_063).millis_since_discord_epoch(),
        41_944_705_796
    );
}

#[test]
fn elapsed_time_is_the_difference_between_the_two_timestamps() {
    assert_eq!(
        at(1_000, 4_095).elapsed_until(at(13_345, 0)),
        Duration::from_millis(12_345)
    );
}

#[test]
fn elapsed_time_is_zero_when_the_later_id_is_older() {
    assert_eq!(at(5_000, 0).elapsed_until(at(4_000, 0)), Duration::ZERO);
}

#[test]
fn ids_round_trip_their_raw_value() {
    assert_eq!(UserId::new(42).get(), 42);
    assert_eq!(Snowflake::new(7).get(), 7);
}

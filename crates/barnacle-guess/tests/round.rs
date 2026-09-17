mod common;

use std::time::Duration;

use barnacle_guess::Guess;
use barnacle_guess::RecentShips;
use barnacle_guess::Round;
use barnacle_guess::RoundOptions;
use barnacle_guess::Solve;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use common::at;
use common::book;
use common::index;
use common::ship;
use common::tier;
use rand::SeedableRng;
use rand::rngs::StdRng;

const INVOKER: UserId = UserId::new(10);
const PLAYER: UserId = UserId::new(20);
const MODERATOR: UserId = UserId::new(30);

fn started() -> Round {
    let options = RoundOptions::new(Some(tier(5)), Some(tier(5)), None);
    book(vec![ship("PJSB005", "Kongō", 5, "kongo")], "")
        .draw(
            &options,
            &RecentShips::default(),
            &mut StdRng::seed_from_u64(1),
        )
        .unwrap()
        .start(INVOKER, at(1_000))
}

fn guess(text: &str, author_is_bot: bool) -> Guess<'_> {
    Guess {
        author: PLAYER,
        author_is_bot,
        message: at(4_250),
        text,
    }
}

#[test]
fn a_correct_guess_solves_the_round_with_the_time_since_the_post() {
    assert_eq!(
        started().judge(&guess("kongo", false)),
        Some(Solve {
            winner: PLAYER,
            ship: index("PJSB005"),
            elapsed: Duration::from_millis(3_250),
        })
    );
}

#[test]
fn the_invoker_may_answer_their_own_round() {
    let own = Guess {
        author: INVOKER,
        ..guess("kongo", false)
    };
    assert_eq!(
        started().judge(&own).map(|solve| solve.winner),
        Some(INVOKER)
    );
}

#[test]
fn a_guess_sent_before_the_post_does_not_solve_the_round() {
    let early = Guess {
        message: at(999),
        ..guess("kongo", false)
    };
    assert_eq!(started().judge(&early), None);
}

#[test]
fn guesses_are_cleaned_before_they_are_compared() {
    let round = started();
    assert!(round.judge(&guess("  KONGŌ ", false)).is_some());
    assert!(round.judge(&guess("Kon-go.", false)).is_some());
}

#[test]
fn wrong_empty_and_bot_guesses_do_not_solve_the_round() {
    let round = started();
    assert_eq!(round.judge(&guess("kirishima", false)), None);
    assert_eq!(round.judge(&guess("...", false)), None);
    assert_eq!(round.judge(&guess("kongo", true)), None);
}

#[test]
fn the_invoker_or_a_member_who_manages_messages_may_cancel() {
    let round = started();
    assert!(round.may_cancel(INVOKER, false));
    assert!(round.may_cancel(MODERATOR, true));
    assert!(!round.may_cancel(MODERATOR, false));
}

#[test]
fn a_round_keeps_its_draw_invoker_and_post() {
    let round = started();
    assert_eq!(round.draw().ship(), &index("PJSB005"));
    assert_eq!(round.invoker(), INVOKER);
    assert_eq!(round.posted(), at(1_000));
}

#[test]
fn the_standard_timing_is_twenty_seconds_then_ten_after_the_hint() {
    assert_eq!(
        Timing::STANDARD,
        Timing {
            before_hint: Duration::from_secs(20),
            after_hint: Duration::from_secs(10),
        }
    );
}

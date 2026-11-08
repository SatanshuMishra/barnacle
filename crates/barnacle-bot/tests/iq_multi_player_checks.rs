use barnacle_bot::iq::Attachment;
use barnacle_bot::iq::Checked;
use barnacle_bot::iq::Criterion;
use barnacle_bot::iq::CriterionKind;
use barnacle_bot::iq::Post;
use barnacle_bot::iq::Problem;
use barnacle_bot::iq::check_submission;
use barnacle_bot::iq_text::submission_reply;
use barnacle_guess::UserId;

const POSTER: UserId = UserId::new(1);
const LOCKOUT: &str = "Leaderboard Lockout";

fn criteria() -> Vec<Criterion> {
    vec![
        Criterion {
            name: LOCKOUT.to_owned(),
            description: String::new(),
            points: 8,
            kind: CriterionKind::Multi,
        },
        Criterion {
            name: "Solo".to_owned(),
            description: String::new(),
            points: 5,
            kind: CriterionKind::Single,
        },
    ]
}

fn two_images() -> Vec<Attachment> {
    vec![
        Attachment {
            content_type: Some("image/png".to_owned()),
        },
        Attachment {
            content_type: Some("image/png".to_owned()),
        },
    ]
}

fn checked(text: &str, bots: &[UserId]) -> Checked {
    let attachments = two_images();
    check_submission(
        &Post {
            text,
            attachments: &attachments,
            author: POSTER,
            bots,
        },
        &criteria(),
    )
}

fn problems(checked: &Checked) -> Vec<Problem> {
    match checked {
        Checked::Invalid(problems) => problems.clone(),
        Checked::Valid { .. } => panic!("the post was counted: {checked:?}"),
    }
}

#[test]
fn one_other_player_counts() {
    assert_eq!(
        checked("Leaderboard Lockout <@2>", &[]),
        Checked::Valid {
            criterion: LOCKOUT.to_owned(),
            points: 8,
            partners: vec![UserId::new(2)],
        }
    );
}

#[test]
fn two_other_players_count_in_any_ping_form() {
    assert_eq!(
        checked("<@!2><@3> Leaderboard Lockout", &[]),
        Checked::Valid {
            criterion: LOCKOUT.to_owned(),
            points: 8,
            partners: vec![UserId::new(2), UserId::new(3)],
        }
    );
}

#[test]
fn a_repeated_ping_counts_once() {
    assert_eq!(
        checked("Leaderboard Lockout <@2> <@2>", &[]),
        Checked::Valid {
            criterion: LOCKOUT.to_owned(),
            points: 8,
            partners: vec![UserId::new(2)],
        }
    );
}

#[test]
fn no_pings_are_refused_with_the_reason() {
    let result = checked("Leaderboard Lockout", &[]);
    assert_eq!(
        result,
        Checked::Invalid(vec![Problem::NoPartners {
            criterion: LOCKOUT.to_owned()
        }])
    );
    let reply = submission_reply(&problems(&result));
    assert!(
        reply.contains("Leaderboard Lockout is a multi-player criterion. Ping the 1 or 2 players you played with after the name."),
        "{reply}"
    );
}

#[test]
fn pinging_only_yourself_or_a_bot_is_refused() {
    let no_partners = Checked::Invalid(vec![Problem::NoPartners {
        criterion: LOCKOUT.to_owned(),
    }]);
    assert_eq!(checked("Leaderboard Lockout <@1>", &[]), no_partners);
    assert_eq!(
        checked("Leaderboard Lockout <@9>", &[UserId::new(9)]),
        no_partners
    );
}

#[test]
fn three_other_players_are_refused() {
    let result = checked("Leaderboard Lockout <@2> <@3> <@4>", &[]);
    assert_eq!(
        result,
        Checked::Invalid(vec![Problem::TooManyPlayers {
            criterion: LOCKOUT.to_owned()
        }])
    );
    let reply = submission_reply(&problems(&result));
    assert!(
        reply.contains("Leaderboard Lockout counts 2 or 3 players, so ping at most 2 others."),
        "{reply}"
    );
}

#[test]
fn a_ping_on_a_single_player_criterion_is_refused() {
    let pinged = checked("Solo <@2>", &[]);
    assert_eq!(
        pinged,
        Checked::Invalid(vec![Problem::SinglePlayerPing {
            criterion: "Solo".to_owned()
        }])
    );
    let reply = submission_reply(&problems(&pinged));
    assert!(
        reply.contains("Solo is a single-player criterion. Write just the name, with no pings."),
        "{reply}"
    );
    assert_eq!(
        checked("Solo", &[]),
        Checked::Valid {
            criterion: "Solo".to_owned(),
            points: 5,
            partners: vec![],
        }
    );
}

#[test]
fn a_mistyped_name_is_judged_without_its_pings() {
    assert_eq!(
        checked("Leaderbord Lockout <@2>", &[]),
        Checked::Invalid(vec![Problem::UnknownName {
            typed: "Leaderbord Lockout".to_owned(),
            closest: LOCKOUT.to_owned(),
        }])
    );
}

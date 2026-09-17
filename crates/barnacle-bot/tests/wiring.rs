mod common;

use barnacle_bot::attendance::Delivery;
use barnacle_bot::attendance::Target;
use barnacle_bot::ids::RoleId;
use barnacle_bot::schedule::Hour;
use barnacle_bot::schedule::Night;
use barnacle_bot::solves::Ranking;
use barnacle_bot::wiring;
use barnacle_bot::wiring::ChannelAccess;
use barnacle_bot::wiring::LeaderboardRequest;
use barnacle_bot::wiring::SignupClick;
use barnacle_bot::wiring::SortChoice;
use barnacle_guess::RoundOptions;
use common::tier;

#[test]
fn command_options_become_round_options() {
    assert_eq!(
        wiring::round_options(None, None, None),
        RoundOptions::default()
    );
    assert_eq!(
        wiring::round_options(Some(9), Some(4), Some(true)),
        RoundOptions::new(Some(tier(9)), Some(tier(4)), Some(true))
    );
    assert_eq!(
        wiring::round_options(Some(0), Some(12), None),
        RoundOptions::default()
    );
}

#[test]
fn a_cancel_button_id_carries_its_round_number() {
    assert_eq!(
        wiring::round_number(&wiring::cancel_button_id(42)),
        Some(42)
    );
    assert_eq!(wiring::round_number(wiring::ENDED_BUTTON_ID), None);
    assert_eq!(wiring::round_number("other-button:42"), None);
    assert_eq!(wiring::round_number("barnacle-cancel:-1"), None);
}

#[test]
fn missing_channel_permissions_are_named_in_order() {
    let full = ChannelAccess {
        view_channel: true,
        send_messages: true,
        embed_links: true,
        attach_files: true,
        read_message_history: true,
        in_thread: false,
    };
    assert!(wiring::missing_permissions(full).is_empty());
    assert_eq!(
        wiring::missing_permissions(ChannelAccess {
            attach_files: false,
            read_message_history: false,
            ..full
        }),
        ["Attach Files", "Read Message History"]
    );
    assert_eq!(
        wiring::missing_permissions(ChannelAccess::default()),
        [
            "View Channel",
            "Send Messages",
            "Embed Links",
            "Attach Files",
            "Read Message History"
        ]
    );
}

#[test]
fn a_thread_names_the_thread_sending_permission() {
    let thread = ChannelAccess {
        view_channel: true,
        send_messages: false,
        embed_links: true,
        attach_files: true,
        read_message_history: true,
        in_thread: true,
    };
    assert_eq!(
        wiring::missing_permissions(thread),
        ["Send Messages in Threads"]
    );
}

#[test]
fn leaderboard_options_default_to_ten_players_by_most_wins() {
    assert_eq!(
        wiring::leaderboard_request(None, None),
        LeaderboardRequest {
            ranking: Ranking::MostWins,
            size: 10
        }
    );
    assert_eq!(
        wiring::leaderboard_request(Some(SortChoice::FastestTime), Some(50)),
        LeaderboardRequest {
            ranking: Ranking::FastestTime,
            size: 50
        }
    );
    assert_eq!(
        wiring::leaderboard_request(Some(SortChoice::MostWins), Some(5)),
        LeaderboardRequest {
            ranking: Ranking::MostWins,
            size: 5
        }
    );
}

const BUTTON_ID_LIMIT: usize = 100;

fn night() -> Night {
    Night::parse("2026-09-23").unwrap()
}

fn targets() -> impl Iterator<Item = Target> {
    std::iter::once(Target::All).chain(Hour::ALL.map(Target::One))
}

#[test]
fn signup_ids_round_trip() {
    assert_eq!(
        wiring::signup_tag_prefix(3, night()),
        "barnacle-cb:3:2026-09-23:"
    );
    assert_eq!(
        wiring::signup_button_id(3, night(), Target::All, true),
        "barnacle-cb:3:2026-09-23:all:in"
    );
    assert_eq!(
        wiring::signup_button_id(3, night(), Target::One(Hour::ALL[3]), false),
        "barnacle-cb:3:2026-09-23:4:out"
    );
    for target in targets() {
        for attending in [true, false] {
            let id = wiring::signup_button_id(3, night(), target, attending);
            assert!(
                id.starts_with(&wiring::signup_tag_prefix(3, night())),
                "{id}"
            );
            assert_eq!(
                wiring::signup_click(&id),
                Some(SignupClick {
                    season: 3,
                    night: night(),
                    target,
                    attending
                }),
                "{id}"
            );
        }
    }
}

#[test]
fn a_malformed_signup_id_is_rejected() {
    for id in [
        "",
        "barnacle-cb:",
        "barnacle-cb:3:2026-09-23:all",
        "barnacle-cb:3:2026-09-23:all:in:1",
        "barnacle-cb:3:2026-09-23:5:in",
        "barnacle-cb:3:2026-09-23:0:in",
        "barnacle-cb:3:2026-09-23:hour:in",
        "barnacle-cb:3:2026-09-23:all:maybe",
        "barnacle-cb:3:2026-09-23:all:IN",
        "barnacle-cb:0:2026-09-23:all:in",
        "barnacle-cb:-3:2026-09-23:all:in",
        "barnacle-cb:+3:2026-09-23:all:in",
        "barnacle-cb:003:2026-09-23:all:in",
        "barnacle-cb:three:2026-09-23:all:in",
        "barnacle-cb:3::all:in",
        "barnacle-cb:3:2026-09-25:all:in",
        "barnacle-cb:3:2026-9-23:all:in",
        "barnacle-cb:3:2026-09-31:all:in",
        "barnacle-cancel:3:2026-09-23:all:in",
    ] {
        assert_eq!(wiring::signup_click(id), None, "{id}");
    }
    assert_eq!(
        wiring::round_number("barnacle-cb:3:2026-09-23:all:in"),
        None
    );
}

#[test]
fn the_longest_signup_id_fits_discords_limit() {
    for target in targets() {
        for attending in [true, false] {
            let id = wiring::signup_button_id(i64::MAX, night(), target, attending);
            assert!(id.chars().count() < BUTTON_ID_LIMIT, "{id}");
            assert_eq!(
                wiring::signup_click(&id).map(|click| click.season),
                Some(i64::MAX),
                "{id}"
            );
        }
    }
}

#[test]
fn a_signup_channel_needs_four_permissions() {
    let full = ChannelAccess {
        view_channel: true,
        send_messages: true,
        embed_links: true,
        attach_files: false,
        read_message_history: true,
        in_thread: false,
    };
    assert!(wiring::missing_signup_permissions(full).is_empty());
    assert_eq!(
        wiring::missing_signup_permissions(ChannelAccess::default()),
        [
            "View Channel",
            "Send Messages",
            "Embed Links",
            "Read Message History"
        ]
    );
    assert_eq!(
        wiring::missing_signup_permissions(ChannelAccess {
            embed_links: false,
            ..full
        }),
        ["Embed Links"]
    );
}

#[test]
fn a_ping_role_renders_as_a_role_mention() {
    assert_eq!(wiring::ping_content(Some(RoleId::new(123))), "<@&123>");
    assert_eq!(wiring::ping_content(None), "");
}

#[test]
fn only_a_new_post_may_ping() {
    let role = RoleId::new(123);
    assert_eq!(wiring::ping_allowance(Delivery::New, Some(role)), [role]);
    assert!(wiring::ping_allowance(Delivery::New, None).is_empty());
    assert!(
        wiring::ping_allowance(Delivery::Redraw, Some(role)).is_empty(),
        "a redraw may never ping"
    );
    assert!(wiring::ping_allowance(Delivery::Redraw, None).is_empty());
}

mod common;

use barnacle_bot::attendance::Delivery;
use barnacle_bot::attendance::Target;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::Ping;
use barnacle_bot::ids::RoleId;
use barnacle_bot::schedule::Hour;
use barnacle_bot::schedule::Night;
use barnacle_bot::solves::Ranking;
use barnacle_bot::wiring;
use barnacle_bot::wiring::ChannelAccess;
use barnacle_bot::wiring::LeaderboardRequest;
use barnacle_bot::wiring::PingAllowance;
use barnacle_bot::wiring::PingChoiceProblem;
use barnacle_bot::wiring::PingReach;
use barnacle_bot::wiring::PingVerdict;
use barnacle_bot::wiring::SetupPing;
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
    assert!(wiring::signup_click("barnacle-cb:3:2026-09-25:all:in").is_some());
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
    assert_eq!(
        wiring::ping_content(&[Ping::Role(RoleId::new(123))]),
        "<@&123>"
    );
    assert_eq!(wiring::ping_content(&[]), "");
}

#[test]
fn only_a_new_post_may_ping() {
    let role = RoleId::new(123);
    assert_eq!(
        wiring::ping_allowance(Delivery::New, &[Ping::Role(role)]),
        PingAllowance {
            everyone: false,
            roles: vec![role],
        }
    );
    assert_eq!(
        wiring::ping_allowance(Delivery::New, &[]),
        PingAllowance::default()
    );
    assert_eq!(
        wiring::ping_allowance(Delivery::Redraw, &[Ping::Role(role)]),
        PingAllowance::default(),
        "a redraw may never ping"
    );
    assert_eq!(
        wiring::ping_allowance(Delivery::Redraw, &[]),
        PingAllowance::default()
    );
}

#[test]
fn an_everyone_ping_allows_everyone_only_on_a_new_post() {
    assert_eq!(
        wiring::ping_allowance(Delivery::New, &[Ping::Everyone]),
        PingAllowance {
            everyone: true,
            roles: vec![],
        }
    );
    assert_eq!(
        wiring::ping_allowance(Delivery::Redraw, &[Ping::Everyone]),
        PingAllowance::default(),
        "a redraw may never ping everyone"
    );
    assert_eq!(wiring::ping_content(&[Ping::Everyone]), "@everyone");
}

#[test]
fn ping_of_names_everyone_by_the_server_id() {
    assert_eq!(Ping::of(GuildId::new(7), RoleId::new(7)), Ping::Everyone);
    assert_eq!(
        Ping::of(GuildId::new(7), RoleId::new(8)),
        Ping::Role(RoleId::new(8))
    );
}

fn reach(role_mentionable: bool, server_grant: bool, channel_grant: bool) -> PingReach {
    PingReach {
        role_mentionable,
        server_grant,
        channel_grant,
    }
}

fn every_reach() -> Vec<PingReach> {
    [false, true]
        .into_iter()
        .flat_map(|mentionable| {
            [false, true].into_iter().flat_map(move |server| {
                [false, true]
                    .into_iter()
                    .map(move |channel| reach(mentionable, server, channel))
            })
        })
        .collect()
}

#[test]
fn a_named_ping_sounds_when_mentionable_or_permitted() {
    let role = Ping::Role(RoleId::new(456));
    for mentionable in every_reach()
        .into_iter()
        .filter(|reach| reach.role_mentionable)
    {
        assert_eq!(
            wiring::ping_verdict(role, Some(mentionable)),
            PingVerdict::Sounds,
            "{mentionable:?}"
        );
    }
    for permitted in every_reach()
        .into_iter()
        .filter(|reach| reach.channel_grant)
    {
        assert_eq!(
            wiring::ping_verdict(role, Some(permitted)),
            PingVerdict::Sounds,
            "{permitted:?}"
        );
    }
}

#[test]
fn a_silent_ping_blames_the_channel_only_when_the_server_grants_it() {
    let role = Ping::Role(RoleId::new(456));
    assert_eq!(
        wiring::ping_verdict(role, Some(reach(false, true, false))),
        PingVerdict::BlockedByChannel
    );
    assert_eq!(
        wiring::ping_verdict(role, Some(reach(false, false, false))),
        PingVerdict::Silent
    );
}

#[test]
fn everyone_needs_the_permission_even_when_marked_mentionable() {
    for mentionable in [false, true] {
        assert_eq!(
            wiring::ping_verdict(Ping::Everyone, Some(reach(mentionable, false, true))),
            PingVerdict::Sounds
        );
        assert_eq!(
            wiring::ping_verdict(Ping::Everyone, Some(reach(mentionable, true, true))),
            PingVerdict::Sounds
        );
        assert_eq!(
            wiring::ping_verdict(Ping::Everyone, Some(reach(mentionable, true, false))),
            PingVerdict::BlockedByChannel
        );
        assert_eq!(
            wiring::ping_verdict(Ping::Everyone, Some(reach(mentionable, false, false))),
            PingVerdict::Silent
        );
    }
}

#[test]
fn an_unreadable_reach_is_unchecked() {
    assert_eq!(
        wiring::ping_verdict(Ping::Role(RoleId::new(456)), None),
        PingVerdict::Unchecked
    );
    assert_eq!(
        wiring::ping_verdict(Ping::Everyone, None),
        PingVerdict::Unchecked
    );
}

#[test]
fn only_a_create_refuses_and_only_for_a_channel_override() {
    assert_eq!(
        wiring::setup_ping(true, PingVerdict::BlockedByChannel),
        SetupPing::Refuse
    );
    assert_eq!(
        wiring::setup_ping(false, PingVerdict::BlockedByChannel),
        SetupPing::Warn
    );
    for creating in [true, false] {
        assert_eq!(
            wiring::setup_ping(creating, PingVerdict::Silent),
            SetupPing::Warn
        );
        assert_eq!(
            wiring::setup_ping(creating, PingVerdict::Unchecked),
            SetupPing::Warn
        );
        assert_eq!(
            wiring::setup_ping(creating, PingVerdict::Sounds),
            SetupPing::Proceed
        );
    }
}

#[test]
fn a_ping_is_heard_only_when_discord_lists_it() {
    let role = RoleId::new(456);
    let other = RoleId::new(789);
    assert!(wiring::unheard_pings(&[], &[], false).is_empty());
    assert!(wiring::unheard_pings(&[], &[role], true).is_empty());
    assert!(wiring::unheard_pings(&[Ping::Role(role)], &[other, role], false).is_empty());
    assert!(!wiring::unheard_pings(&[Ping::Role(role)], &[other], true).is_empty());
    assert!(!wiring::unheard_pings(&[Ping::Role(role)], &[], true).is_empty());
    assert!(wiring::unheard_pings(&[Ping::Everyone], &[], true).is_empty());
    assert!(!wiring::unheard_pings(&[Ping::Everyone], &[role], false).is_empty());
}

#[test]
fn several_pings_are_written_in_order_and_allowed_together() {
    let pings = [Ping::Role(RoleId::new(1)), Ping::Role(RoleId::new(2))];
    assert_eq!(wiring::ping_content(&pings), "<@&1> <@&2>");
    assert_eq!(
        wiring::ping_allowance(Delivery::New, &pings),
        PingAllowance {
            everyone: false,
            roles: vec![RoleId::new(1), RoleId::new(2)],
        }
    );
    assert_eq!(
        wiring::ping_allowance(Delivery::Redraw, &pings),
        PingAllowance::default()
    );
}

#[test]
fn unheard_pings_lists_each_ping_discord_missed() {
    let one = RoleId::new(1);
    let two = RoleId::new(2);
    let pings = [Ping::Role(one), Ping::Role(two)];
    assert_eq!(
        wiring::unheard_pings(&pings, &[one], false),
        vec![Ping::Role(two)]
    );
    assert_eq!(wiring::unheard_pings(&pings, &[one, two], false), vec![]);
    assert_eq!(
        wiring::unheard_pings(&[Ping::Everyone], &[], false),
        vec![Ping::Everyone]
    );
}

#[test]
fn ping_choice_keeps_option_order_and_drops_repeats() {
    let server = GuildId::new(7);
    let a = RoleId::new(8);
    let b = RoleId::new(9);
    assert_eq!(
        wiring::ping_choice(server, [None, Some(a), None, Some(b), Some(a)]),
        Ok(vec![a, b])
    );
    assert_eq!(wiring::ping_choice(server, [None; 5]), Ok(vec![]));
    assert_eq!(wiring::ping_choice(server, [Some(a); 5]), Ok(vec![a]));
}

#[test]
fn ping_choice_refuses_everyone_among_named_roles() {
    let server = GuildId::new(7);
    let everyone = RoleId::new(7);
    assert_eq!(
        wiring::ping_choice(
            server,
            [Some(everyone), Some(RoleId::new(8)), None, None, None]
        ),
        Err(PingChoiceProblem::MixedEveryone)
    );
    assert_eq!(
        wiring::ping_choice(server, [Some(everyone), Some(everyone), None, None, None]),
        Ok(vec![everyone])
    );
}

#[test]
fn ping_again_needs_the_person_to_be_able_to_ping() {
    let roles = [Ping::Role(RoleId::new(4)), Ping::Role(RoleId::new(5))];
    assert!(!wiring::may_ping_again(&roles, None, &[true, true]));
    assert!(wiring::may_ping_again(&roles, Some(true), &[false, false]));
    assert!(wiring::may_ping_again(&roles, Some(false), &[true, true]));
    assert!(!wiring::may_ping_again(&roles, Some(false), &[true, false]));
    assert!(!wiring::may_ping_again(&[Ping::Everyone], None, &[false]));
    assert!(wiring::may_ping_again(
        &[Ping::Everyone],
        Some(true),
        &[false]
    ));
    assert!(!wiring::may_ping_again(
        &[Ping::Everyone],
        Some(false),
        &[false]
    ));
    assert!(!wiring::may_ping_again(
        &[Ping::Everyone],
        Some(false),
        &[true]
    ));
}

mod common;

use std::collections::HashMap;

use barnacle_bot::attendance::Delivery;
use barnacle_bot::attendance::Target;
use barnacle_bot::attendance_store::Attendance;
use barnacle_bot::attendance_store::CreateOutcome;
use barnacle_bot::attendance_store::Event;
use barnacle_bot::attendance_store::NewEvent;
use barnacle_bot::discord;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::ids::Ping;
use barnacle_bot::ids::RoleId;
use barnacle_bot::schedule::Hour;
use barnacle_bot::schedule::Hours;
use barnacle_bot::schedule::Schedule;
use barnacle_bot::schedule::Span;
use barnacle_bot::schedule::StartTime;
use barnacle_bot::schedule::Weekdays;
use barnacle_bot::schedule::Zone;
use barnacle_bot::schedule::parse_day;
use barnacle_bot::solves::Ranking;
use barnacle_bot::wiring;
use barnacle_bot::wiring::ChannelAccess;
use barnacle_bot::wiring::ChannelProblem;
use barnacle_bot::wiring::EventChoice;
use barnacle_bot::wiring::LeaderboardRequest;
use barnacle_bot::wiring::PanelPart;
use barnacle_bot::wiring::PingAllowance;
use barnacle_bot::wiring::PingChoiceProblem;
use barnacle_bot::wiring::PingReach;
use barnacle_bot::wiring::PingVerdict;
use barnacle_bot::wiring::SeriesClick;
use barnacle_bot::wiring::SessionRef;
use barnacle_bot::wiring::SetupPing;
use barnacle_bot::wiring::SignupClick;
use barnacle_bot::wiring::SortChoice;
use barnacle_guess::RoundOptions;
use barnacle_guess::SeriesLength;
use barnacle_guess::Snowflake;
use barnacle_guess::UserId;
use common::tier;
use jiff::civil::Date;
use poise::serenity_prelude as serenity;

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
fn series_options_become_a_series_length() {
    assert_eq!(
        wiring::series_request(None, None, None, None),
        Some((RoundOptions::default(), SeriesLength::default()))
    );
    assert_eq!(
        wiring::series_request(None, None, None, None).map(|(_, length)| length.get()),
        Some(10)
    );
    assert_eq!(
        wiring::series_request(Some(9), Some(4), Some(true), Some(7)),
        Some((
            wiring::round_options(Some(9), Some(4), Some(true)),
            SeriesLength::new(7).unwrap()
        ))
    );
    assert_eq!(
        wiring::series_request(Some(0), Some(12), None, Some(2)).map(|(options, _)| options),
        Some(wiring::round_options(Some(0), Some(12), None))
    );
    assert_eq!(
        wiring::series_request(None, None, None, Some(20)).map(|(_, length)| length.get()),
        Some(20)
    );
    for rounds in [1, 21, 0, -3, i64::MAX] {
        assert_eq!(
            wiring::series_request(None, None, None, Some(rounds)),
            None,
            "{rounds}"
        );
    }
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
fn series_button_ids_round_trip_and_fit_discords_limit() {
    for number in [0, 42, u64::MAX] {
        let built = [
            (wiring::skip_button_id(number), SeriesClick::Skip(number)),
            (
                wiring::end_series_button_id(number),
                SeriesClick::End(number),
            ),
            (
                wiring::still_playing_button_id(number),
                SeriesClick::StillPlaying(number),
            ),
        ];
        for (id, click) in built {
            assert_eq!(wiring::series_click(&id), Some(click), "{id}");
            assert_eq!(wiring::round_number(&id), None, "{id}");
            assert!(id.chars().count() <= BUTTON_ID_LIMIT, "{id}");
        }
    }
    assert_eq!(wiring::skip_button_id(7), "barnacle-skip:7");
    assert_eq!(wiring::end_series_button_id(7), "barnacle-end:7");
    assert_eq!(wiring::still_playing_button_id(7), "barnacle-still:7");
    for id in [
        wiring::ENDED_SKIP_BUTTON_ID,
        wiring::ENDED_END_BUTTON_ID,
        wiring::ENDED_BUTTON_ID,
        &wiring::cancel_button_id(5),
        &wiring::signup_button_id(3, date(), Target::All, true),
        "barnacle-skip:",
        "barnacle-skip:-1",
        "barnacle-end:five",
        "barnacle-still:5:6",
        "other-button:5",
    ] {
        assert_eq!(wiring::series_click(id), None, "{id}");
    }
}

#[test]
fn an_announce_button_id_carries_its_version() {
    assert_eq!(
        wiring::announce_button_id("0.2.0"),
        "barnacle-announce:0.2.0"
    );
    assert_eq!(
        wiring::announce_version(&wiring::announce_button_id("0.2.0")),
        Some("0.2.0".to_owned())
    );
    let long = "1234567890.1234567890.1234567890";
    assert_eq!(long.len(), 32);
    let id = wiring::announce_button_id(long);
    assert!(id.chars().count() <= BUTTON_ID_LIMIT, "{id}");
    assert_eq!(wiring::announce_version(&id), Some(long.to_owned()));
    for other in [
        wiring::skip_button_id(5),
        wiring::end_series_button_id(5),
        wiring::still_playing_button_id(5),
        wiring::ENDED_SKIP_BUTTON_ID.to_owned(),
        wiring::cancel_button_id(5),
        wiring::ENDED_BUTTON_ID.to_owned(),
        wiring::signup_button_id(3, date(), Target::All, true),
        "barnacle-announce:".to_owned(),
    ] {
        assert_eq!(wiring::announce_version(&other), None, "{other}");
    }
}

#[test]
fn an_updates_channel_needs_three_permissions() {
    let full = ChannelAccess {
        view_channel: true,
        send_messages: true,
        embed_links: true,
        attach_files: false,
        read_message_history: false,
        in_thread: false,
    };
    assert!(wiring::missing_update_permissions(full).is_empty());
    assert_eq!(
        wiring::missing_update_permissions(ChannelAccess::default()),
        ["View Channel", "Send Messages", "Embed Links"]
    );
    assert_eq!(
        wiring::missing_update_permissions(ChannelAccess {
            embed_links: false,
            ..full
        }),
        ["Embed Links"]
    );
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

fn date() -> Date {
    parse_day("2026-09-23").unwrap()
}

fn targets() -> impl Iterator<Item = Target> {
    std::iter::once(Target::All).chain(Hours::new(24).unwrap().each().map(Target::One))
}

#[test]
fn signup_ids_round_trip() {
    assert_eq!(
        wiring::signup_tag_prefix(3, date()),
        "barnacle-cb:3:2026-09-23:"
    );
    assert_eq!(
        wiring::signup_button_id(3, date(), Target::All, true),
        "barnacle-cb:3:2026-09-23:all:in"
    );
    assert_eq!(
        wiring::signup_button_id(3, date(), Target::One(Hour::new(4).unwrap()), false),
        "barnacle-cb:3:2026-09-23:4:out"
    );
    for target in targets() {
        for attending in [true, false] {
            let id = wiring::signup_button_id(3, date(), target, attending);
            assert!(
                id.starts_with(&wiring::signup_tag_prefix(3, date())),
                "{id}"
            );
            assert_eq!(
                wiring::signup_click(&id),
                Some(SignupClick {
                    event: 3,
                    date: date(),
                    target,
                    attending
                }),
                "{id}"
            );
        }
    }
}

#[test]
fn a_button_names_its_event_date_hour_and_answer() {
    let date = parse_day("2026-10-14").unwrap();
    assert_eq!(
        wiring::signup_click("barnacle-cb:7:2026-10-14:all:in"),
        Some(SignupClick {
            event: 7,
            date,
            target: Target::All,
            attending: true
        })
    );
    assert_eq!(
        wiring::signup_click("barnacle-cb:7:2026-10-14:all:out"),
        Some(SignupClick {
            event: 7,
            date,
            target: Target::All,
            attending: false
        })
    );
    assert_eq!(
        wiring::signup_click("barnacle-cb:7:2026-10-14:12:out"),
        Some(SignupClick {
            event: 7,
            date,
            target: Target::One(Hour::new(12).unwrap()),
            attending: false
        })
    );
}

#[test]
fn a_malformed_button_is_ignored() {
    for id in [
        "barnacle-cb:7:2026-10-14:25:in",
        "barnacle-cb:07:2026-10-14:all:in",
        "barnacle-cb:7:2026-13-01:all:in",
        "barnacle-cb:7:2026-10-14:all",
    ] {
        assert_eq!(wiring::signup_click(id), None, "{id}");
    }
}

#[test]
fn the_choose_hours_ids_name_their_event_and_date() {
    let october_14 = parse_day("2026-10-14").unwrap();
    let pick = wiring::choose_button_id(7, october_14);
    let menu = wiring::hours_menu_id(7, october_14);
    assert_eq!(pick, "barnacle-cb-pick:7:2026-10-14");
    assert_eq!(menu, "barnacle-cb-hours:7:2026-10-14");
    let expected = Some(SessionRef {
        event: 7,
        date: october_14,
    });
    assert_eq!(wiring::choose_press(&pick), expected);
    assert_eq!(wiring::hours_menu(&menu), expected);
    assert_eq!(wiring::choose_press(&menu), None);
    assert_eq!(wiring::hours_menu(&pick), None);
}

#[test]
fn a_malformed_choose_hours_id_is_ignored() {
    for id in [
        "barnacle-cb-pick:07:2026-10-14",
        "barnacle-cb-pick:0:2026-10-14",
        "barnacle-cb-pick:7:2026-10-14:x",
        "barnacle-cb-pick:7",
        "barnacle-cb-hours:7:2026-13-01",
        "barnacle-cb-hours:7:14-10-2026",
        "barnacle-cb-hours:",
        "barnacle-cb:7:2026-10-14:all:in",
    ] {
        assert_eq!(wiring::choose_press(id), None, "{id}");
        assert_eq!(wiring::hours_menu(id), None, "{id}");
    }
    assert_eq!(
        wiring::signup_click("barnacle-cb:7:2026-10-14:all:in"),
        Some(SignupClick {
            event: 7,
            date: parse_day("2026-10-14").unwrap(),
            target: Target::All,
            attending: true
        })
    );
    assert_eq!(wiring::signup_click("barnacle-cb-pick:7:2026-10-14"), None);
    assert_eq!(wiring::signup_click("barnacle-cb-hours:7:2026-10-14"), None);
}

#[test]
fn hour_choices_keep_hours_of_the_event_once_each() {
    let six = Hours::new(6).unwrap();
    let values = |texts: &[&str]| {
        texts
            .iter()
            .map(|text| (*text).to_owned())
            .collect::<Vec<String>>()
    };
    let hours = |numbers: &[u8]| {
        numbers
            .iter()
            .map(|number| Hour::new(*number).unwrap())
            .collect::<Vec<Hour>>()
    };
    assert_eq!(
        wiring::hour_choices(&values(&["5", "2", "3"]), six),
        Some(hours(&[2, 3, 5]))
    );
    assert_eq!(wiring::hour_choices(&[], six), Some(Vec::new()));
    for bad in [&["7"][..], &["x"], &["2", "2"], &["02"], &["0"]] {
        assert_eq!(wiring::hour_choices(&values(bad), six), None, "{bad:?}");
    }
}

#[test]
fn an_event_name_is_trimmed_and_holds_1_to_100_characters() {
    assert_eq!(
        wiring::event_name("  Ranked Sprint  "),
        Some("Ranked Sprint".to_owned())
    );
    assert_eq!(wiring::event_name("   "), None);
    assert_eq!(wiring::event_name(""), None);
    let longest = "é".repeat(100);
    assert_eq!(wiring::event_name(&longest), Some(longest.clone()));
    assert_eq!(wiring::event_name(&"é".repeat(101)), None);
}

#[test]
fn names_match_after_lowercasing_with_unicode_rules() {
    assert!(wiring::same_name("Été", "été"));
    assert!(wiring::same_name("RANKED Sprint", "ranked sprint"));
    assert!(!wiring::same_name("Ranked Sprint", "Ranked"));
}

#[test]
fn a_malformed_signup_id_is_rejected() {
    for id in [
        "",
        "barnacle-cb:",
        "barnacle-cb:3:2026-09-23:all",
        "barnacle-cb:3:2026-09-23:all:in:1",
        "barnacle-cb:3:2026-09-23:25:in",
        "barnacle-cb:3:2026-09-23:05:in",
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
            let id = wiring::signup_button_id(i64::MAX, date(), target, attending);
            assert!(id.chars().count() < BUTTON_ID_LIMIT, "{id}");
            assert_eq!(
                wiring::signup_click(&id).map(|click| click.event),
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

const SERVER: GuildId = GuildId::new(1);
const BEFORE_THE_SESSIONS: i64 = 1_791_900_000;
const AFTER_THE_QUALIFIER: i64 = 1_792_400_000;

fn every_day() -> Schedule {
    Schedule::new(
        Weekdays::EVERY_DAY,
        StartTime::new(23, 30).unwrap(),
        Zone::utc(),
        Hours::new(4).unwrap(),
    )
}

fn stored(id: i64, name: &str, first_day: Date, last_day: &str, schedule: Schedule) -> Event {
    Event {
        id,
        guild: SERVER,
        channel: ChannelId::new(10),
        name: name.to_owned(),
        span: Span::new(first_day, parse_day(last_day).unwrap(), schedule).unwrap(),
        created_by: UserId::new(100),
        created_at_ms: 0,
        ping_roles: Vec::new(),
        ended_at_ms: None,
        posts_up: false,
    }
}

fn running(id: i64, name: &str, first_day: &str, last_day: &str) -> Event {
    stored(
        id,
        name,
        parse_day(first_day).unwrap(),
        last_day,
        Schedule::clan_battles(),
    )
}

fn choice(label: &str, value: &str) -> EventChoice {
    EventChoice {
        label: label.to_owned(),
        value: value.to_owned(),
    }
}

fn picked(events: &[Event], text: &str, now_unix: i64) -> Option<i64> {
    wiring::chosen_event(events, text, now_unix).map(|event| event.id)
}

#[test]
fn typing_part_of_a_name_suggests_matching_events() {
    let events = [
        running(3, "Clan Battles · Season 35", "2026-09-16", "2026-11-05"),
        running(4, "Ranked Sprint", "2026-10-14", "2026-10-18"),
    ];
    assert_eq!(
        wiring::event_choices(&events, "RANK", BEFORE_THE_SESSIONS),
        vec![choice("Ranked Sprint · 2026-10-14 to 2026-10-18", "4")]
    );
}

#[test]
fn typing_an_accented_name_in_other_case_suggests_it() {
    let events = [running(11, "Été Cup", "2026-10-14", "2026-10-18")];
    assert_eq!(
        wiring::event_choices(&events, "été", BEFORE_THE_SESSIONS),
        vec![choice("Été Cup · 2026-10-14 to 2026-10-18", "11")]
    );
}

#[test]
fn an_empty_query_lists_the_earliest_25_events() {
    let long_name = "Autumn Ranked Sprint for the Reserve Crews of Every Allied Clan across the North American, European and Asian Server Regions, Week One";
    assert_eq!(long_name.chars().count(), 134);
    let opening = parse_day("2026-10-14").unwrap();
    let events: Vec<Event> = (1..=30)
        .map(|id| {
            let first_day = opening
                .checked_add(jiff::Span::new().days(30 - id))
                .unwrap();
            let name = if id == 25 {
                long_name.to_owned()
            } else {
                format!("Ranked Sprint {id}")
            };
            stored(id, &name, first_day, "2026-12-31", every_day())
        })
        .collect();
    let choices = wiring::event_choices(&events, "", BEFORE_THE_SESSIONS);
    let values: Vec<&str> = choices.iter().map(|choice| choice.value.as_str()).collect();
    assert_eq!(
        values,
        [
            "30", "29", "28", "27", "26", "25", "24", "23", "22", "21", "20", "19", "18", "17",
            "16", "15", "14", "13", "12", "11", "10", "9", "8", "7", "6"
        ]
    );
    assert_eq!(
        choices[0],
        choice("Ranked Sprint 30 · 2026-10-14 to 2026-12-31", "30")
    );
    assert_eq!(
        choices[24],
        choice("Ranked Sprint 6 · 2026-11-07 to 2026-12-31", "6")
    );
    assert_eq!(
        choices[5],
        choice(
            "Autumn Ranked Sprint for the Reserve Crews of Every Allied Clan across t… · 2026-10-19 to 2026-12-31",
            "25"
        )
    );
    assert_eq!(choices[5].label.chars().count(), 100);
    assert!(choices[5].label.ends_with(" · 2026-10-19 to 2026-12-31"));
    assert!(
        choices
            .iter()
            .all(|choice| choice.label.chars().count() <= 100)
    );
}

#[tokio::test]
async fn a_failed_read_suggests_no_event() {
    let pool = common::attendance_pool().await;
    let store = Attendance::with_pool(pool.clone()).await.unwrap();
    sqlx::query("INSERT INTO cb_seasons (id, guild_id, channel_id, name, first_day, last_day, weekdays, start_minute, time_zone, hours, created_by, created_at_ms) VALUES (1, 1, 10, 'Ranked Sprint', '2026-10-14', '2026-10-18', 108, 1410, 'Nowhere/City', 4, 100, 0)")
        .execute(&pool)
        .await
        .unwrap();
    let read = store.events_in(SERVER).await;
    assert!(read.is_err());
    assert_eq!(
        wiring::event_suggestions(read, "", BEFORE_THE_SESSIONS),
        Vec::new()
    );
}

#[test]
fn a_finished_event_is_not_suggested() {
    let events = [
        running(4, "Ranked Sprint", "2026-10-14", "2026-10-25"),
        running(6, "Ranked Qualifier", "2026-10-14", "2026-10-14"),
    ];
    assert_eq!(
        events[1].span.sessions().last().unwrap().remove_at_unix(),
        1_792_036_800
    );
    assert_eq!(
        wiring::event_choices(&events, "rank", AFTER_THE_QUALIFIER),
        vec![choice("Ranked Sprint · 2026-10-14 to 2026-10-25", "4")]
    );
}

#[test]
fn a_query_with_no_match_suggests_nothing() {
    assert_eq!(
        wiring::event_choices(&[], "", BEFORE_THE_SESSIONS),
        Vec::new()
    );
    let events = [
        running(4, "Ranked Sprint", "2026-10-14", "2026-10-18"),
        running(9, &"a".repeat(120), "2026-10-14", "2026-10-18"),
    ];
    assert_eq!(
        wiring::event_choices(&events, "zzz", BEFORE_THE_SESSIONS),
        Vec::new()
    );
    assert_eq!(
        wiring::event_choices(&events, &"a".repeat(101), BEFORE_THE_SESSIONS),
        Vec::new()
    );
}

#[test]
fn a_suggested_id_picks_its_event() {
    let events = [
        running(4, "Ranked Sprint", "2026-10-14", "2026-10-18"),
        running(9, "Ranked Qualifier", "2026-10-14", "2026-10-18"),
    ];
    assert_eq!(picked(&events, "9", BEFORE_THE_SESSIONS), Some(9));
}

#[test]
fn a_typed_name_picks_its_event() {
    let events = [
        running(4, "Ranked Sprint", "2026-10-14", "2026-10-18"),
        running(9, "2026", "2026-10-14", "2026-10-18"),
        running(11, "Été Cup", "2026-10-14", "2026-10-18"),
    ];
    assert_eq!(
        picked(&events, " ranked sprint ", BEFORE_THE_SESSIONS),
        Some(4)
    );
    assert_eq!(picked(&events, "2026", BEFORE_THE_SESSIONS), Some(9));
    assert_eq!(picked(&events, "été cup", BEFORE_THE_SESSIONS), Some(11));
}

#[tokio::test]
async fn anything_else_is_not_found() {
    let store = common::attendance().await;
    created(&store, 1, "Ranked Sprint").await;
    created(&store, 1, "Ranked Finals").await;
    let warmup = created(&store, 1, "Ranked Warmup").await;
    store
        .record_post(
            warmup.id,
            parse_day("2026-10-14").unwrap(),
            Snowflake::new(7),
            1_791_800_000_000,
        )
        .await
        .unwrap();
    store
        .end_event(SERVER, warmup.id, 1_791_850_000_000)
        .await
        .unwrap();
    let drill = created(&store, 2, "Drill").await;
    let events = store.events_in(SERVER).await.unwrap();
    assert_eq!(
        events
            .iter()
            .map(|event| event.name.as_str())
            .collect::<Vec<&str>>(),
        ["Ranked Sprint", "Ranked Finals"]
    );
    for text in [
        warmup.id.to_string(),
        drill.id.to_string(),
        "zzz".to_owned(),
        String::new(),
    ] {
        assert_eq!(picked(&events, &text, 1_791_900_000), None, "{text}");
    }
}

async fn created(store: &Attendance, guild: u64, name: &str) -> Event {
    let new = NewEvent {
        guild: GuildId::new(guild),
        channel: ChannelId::new(10),
        name: name.to_owned(),
        span: Span::new(
            parse_day("2026-10-14").unwrap(),
            parse_day("2026-10-18").unwrap(),
            Schedule::clan_battles(),
        )
        .unwrap(),
        created_by: UserId::new(100),
        created_at_ms: 1_791_800_000_000,
        ping_roles: Vec::new(),
    };
    match store.create_event(&new).await.unwrap() {
        CreateOutcome::Created(event) => event,
        CreateOutcome::NameTaken => panic!("{name} was not stored"),
    }
}

#[test]
fn a_finished_event_is_not_found() {
    let events = [running(6, "Ranked Qualifier", "2026-10-14", "2026-10-14")];
    assert_eq!(
        events[0].span.sessions().last().unwrap().remove_at_unix(),
        1_792_036_800
    );
    for text in ["6", "Ranked Qualifier"] {
        assert_eq!(picked(&events, text, AFTER_THE_QUALIFIER), None, "{text}");
    }
}

#[test]
fn a_malformed_panel_id_is_ignored() {
    assert_eq!(
        wiring::panel_part("barnacle-event:12:days"),
        Some((12, PanelPart::Days))
    );
    assert_eq!(
        wiring::panel_part("barnacle-event:12:roles"),
        Some((12, PanelPart::Roles))
    );
    assert_eq!(
        wiring::panel_part("barnacle-event:12:everyone"),
        Some((12, PanelPart::Everyone))
    );
    assert_eq!(
        wiring::panel_part("barnacle-event:12:save"),
        Some((12, PanelPart::Save))
    );
    assert_eq!(
        wiring::panel_part("barnacle-event:12:cancel"),
        Some((12, PanelPart::Cancel))
    );
    for id in [
        "barnacle-event:012:save",
        "barnacle-event:12:launch",
        "barnacle-event:12",
        "barnacle-event:12:save:x",
    ] {
        assert_eq!(wiring::panel_part(id), None, "{id}");
    }
}

#[test]
fn an_edit_channel_must_be_a_text_channel_of_this_server() {
    let channels = HashMap::from([
        cached(12, 1, serenity::ChannelType::Voice),
        cached(13, 2, serenity::ChannelType::Text),
        cached(11, 1, serenity::ChannelType::Text),
    ]);
    let judged = |chosen: u64| {
        wiring::event_channel(
            GuildId::new(1),
            discord::cached_event_channel(&channels, serenity::ChannelId::new(chosen)),
        )
    };
    assert_eq!(judged(14), Err(ChannelProblem::Unknown));
    assert_eq!(judged(13), Err(ChannelProblem::Unknown));
    assert_eq!(judged(12), Err(ChannelProblem::NotText));
    assert_eq!(judged(11), Ok(ChannelId::new(11)));
}

fn cached(
    id: u64,
    guild: u64,
    kind: serenity::ChannelType,
) -> (serenity::ChannelId, serenity::GuildChannel) {
    let mut channel = serenity::GuildChannel::default();
    channel.id = serenity::ChannelId::new(id);
    channel.guild_id = serenity::GuildId::new(guild);
    channel.kind = kind;
    (channel.id, channel)
}

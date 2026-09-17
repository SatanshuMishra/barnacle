mod common;

use barnacle_bot::wiring;
use barnacle_bot::wiring::ChannelAccess;
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

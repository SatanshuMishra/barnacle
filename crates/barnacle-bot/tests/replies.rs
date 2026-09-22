use barnacle_bot::ids::ChannelId;
use barnacle_bot::voice_text;

const HUB: ChannelId = ChannelId::new(10);

#[test]
fn a_hub_that_cannot_open_rooms_names_what_barnacle_lacks() {
    assert_eq!(
        voice_text::cannot_open_rooms(&["Manage Channels"]),
        "Barnacle cannot open rooms from it yet: it lacks Manage Channels. A server admin can grant it to Barnacle's role, or remove it from the channel's permissions."
    );
    assert_eq!(
        voice_text::cannot_open_rooms(&["Manage Channels", "Connect", "Move Members"]),
        "Barnacle cannot open rooms from it yet: it lacks Manage Channels, Connect and Move Members. A server admin can grant them to Barnacle's role, or remove them from the channel's permissions."
    );
    assert_eq!(
        voice_text::hub_line_ready(HUB, "cb", 2, &[]),
        "<#10> opens `cb-#` rooms, 2 open now"
    );
    assert_eq!(
        voice_text::hub_line_ready(HUB, "cb", 2, &[]),
        voice_text::hub_line(HUB, "cb", 2)
    );
    assert_eq!(
        voice_text::hub_line_ready(HUB, "cb", 0, &["Administrator"]),
        "<#10> opens `cb-#` rooms, 0 open now; cannot open rooms: Barnacle lacks Administrator"
    );
    assert_eq!(
        voice_text::hub_line_ready(HUB, "scrim", 1, &["Connect", "Move Members"]),
        "<#10> opens `scrim-#` rooms, 1 open now; cannot open rooms: Barnacle lacks Connect and Move Members"
    );
}

use crate::failure::join_names;
use crate::ids::ChannelId;

pub const HUB_DEFAULT_NAME: &str = "Join to Create";
pub const MOVED_TO_TOP: &str = "moved out of its category";
pub const NO_HUBS: &str =
    "This server has no Join to Create channels. `/voice hub create` makes one.";

pub fn name_too_long(limit: usize) -> String {
    format!("That name is too long; the limit is {limit} characters.")
}

pub fn hub_created(hub: ChannelId, room_name: &str) -> String {
    format!(
        "Created <#{}>. Joining it opens a room named `{room_name}-1`, `{room_name}-2` and so on, and moves the member in. Set who can see and join it in its channel permissions; every room copies them.",
        hub.get()
    )
}

pub fn hub_edited(hub: ChannelId, changes: &[String]) -> String {
    format!(
        "Updated <#{}>: {}. Rooms already open keep their names.",
        hub.get(),
        changes.join("; ")
    )
}

pub fn renamed(name: &str) -> String {
    format!("renamed to {name}")
}

pub fn rooms_renamed(room_name: &str) -> String {
    format!("new rooms are named `{room_name}-1`, `{room_name}-2` and so on")
}

pub fn moved_to(category: ChannelId) -> String {
    format!("moved to <#{}>", category.get())
}

pub fn hub_removed(name: &str) -> String {
    format!(
        "Deleted the Join to Create channel {name}. Rooms it opened stay until they empty, then close as usual."
    )
}

pub fn hub_line(hub: ChannelId, room_name: &str, open: usize) -> String {
    format!(
        "<#{}> opens `{room_name}-#` rooms, {open} open now",
        hub.get()
    )
}

pub fn hub_line_ready(hub: ChannelId, room_name: &str, open: usize, blockers: &[&str]) -> String {
    let line = hub_line(hub, room_name, open);
    if blockers.is_empty() {
        line
    } else {
        format!(
            "{line}; cannot open rooms: Barnacle lacks {}",
            join_names(blockers)
        )
    }
}

pub fn cannot_open_rooms(names: &[&str]) -> String {
    let them = if names.len() == 1 { "it" } else { "them" };
    format!(
        "Barnacle cannot open rooms from it yet: it lacks {}. A server admin can grant {them} to Barnacle's role, or remove {them} from the channel's permissions.",
        join_names(names)
    )
}

pub fn hub_list(lines: &[String]) -> String {
    std::iter::once("Join to Create channels in this server:".to_owned())
        .chain(lines.iter().map(|line| format!("- {line}")))
        .collect::<Vec<_>>()
        .join("\n")
}

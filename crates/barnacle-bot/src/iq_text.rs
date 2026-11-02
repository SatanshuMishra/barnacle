use barnacle_guess::UserId;
use jiff::civil::Date;

use crate::failure::join_names;
use crate::ids::ChannelId;
use crate::ids::RoleId;
use crate::iq::BoardRow;
use crate::iq::Criterion;
use crate::iq::Done;
use crate::iq::JudgingReaction;
use crate::iq::Problem;
use crate::iq::Standing;
use crate::iq_setup::SetupRefusal;
use crate::iq_setup::SetupStep;
use crate::text::day_label;
use crate::text::escape;

const ANNOUNCED_LINES: usize = 5;
const TOP_LINES: usize = 10;
const SHOWN_END_OFFSET: i64 = 60;
const UNKNOWN_MEMBER: &str = "Unknown member";

pub const ONLY_JUDGES: &str = "Only members with @IQ-Judge can change points.";
pub const NO_COMPETITION_RUNNING: &str =
    "No IQ Competition is running. Start one with **/iq start**.";
pub const NEVER_RAN: &str = "No IQ Competition has run in this server yet.";
pub const RESUMED: &str =
    "Resumed. Posts made from now on are checked. Posts from the pause stay unmarked.";
pub const COMPLETED: &str = "Posted. Run /iq cleanup when you are ready to remove the channels.";
pub const CANCELLED: &str = "Cancelled. Nothing changed.";
pub const NO_POINTS_YET: &str = "No points yet.";

pub fn unfinished(done: &[Done]) -> String {
    let steps: Vec<&str> = done.iter().map(|step| done_line(*step)).collect();
    match (done.is_empty(), done.contains(&Done::Completed)) {
        (true, _) => "The IQ Competition was not changed".to_owned(),
        (false, true) => format!(
            "The IQ Competition was only partly changed: {}. The winner announcement was not posted, so post the result by hand, then run /iq cleanup",
            join_names(&steps)
        ),
        (false, false) => format!(
            "The IQ Competition was only partly changed: {}. Press it again to finish the rest",
            join_names(&steps)
        ),
    }
}

fn done_line(step: Done) -> &'static str {
    match step {
        Done::JudgesChanged => "the IQ-Judge role was handed out and taken away",
        Done::ChannelsUpdated => "the channel permissions were updated",
        Done::AnnouncementUpdated => "the announcement was updated",
        Done::Completed => "the competition was marked complete",
        Done::ChannelsDeleted => "the channels were deleted",
    }
}

fn plain_text(typed: &str) -> String {
    escape(&typed.split_whitespace().collect::<Vec<_>>().join(" "))
}

pub fn submission_reply(problems: &[Problem]) -> String {
    let lead = match problems.len() {
        1 => "**Not counted.** One thing to fix:".to_owned(),
        count => format!("**Not counted.** {count} things to fix:"),
    };
    let lines = problems
        .iter()
        .map(|problem| format!("- {}", problem_line(problem)));
    [lead]
        .into_iter()
        .chain(lines)
        .chain(["Post it again as a new message. Edits are not re-checked.".to_owned()])
        .collect::<Vec<_>>()
        .join("\n")
}

fn problem_line(problem: &Problem) -> String {
    match problem {
        Problem::NoName { example } => format!(
            "The message has no criterion name. Write just the name, for example {example}."
        ),
        Problem::UnknownName { typed, closest } => {
            format!(
                "No criterion is called \"{}\". The closest is {closest}.",
                plain_text(typed)
            )
        }
        Problem::ScreenshotCount(count) => {
            let noun = match count {
                1 => "screenshot",
                _ => "screenshots",
            };
            format!("It has {count} {noun}; it needs exactly 2.")
        }
        Problem::NotAnImage => {
            "Only images count as screenshots. Leave other files out.".to_owned()
        }
    }
}

pub fn board_line(row: &BoardRow, name: &str) -> String {
    let rank = match row.rank {
        Some(rank) => format!("{rank:02}"),
        None => "--".to_owned(),
    };
    let line = format!("{rank}. {name} \u{b7} {}", row.points);
    match row.requester {
        true => format!("**{line}**"),
        false => line,
    }
}

pub fn leaderboard(
    competition: &str,
    end_unix: i64,
    now_unix: i64,
    rows: &[(BoardRow, String)],
    players: usize,
) -> String {
    let verb = match now_unix >= end_unix {
        true => "ended",
        false => "ends",
    };
    let title = format!(
        "**{competition} leaderboard** \u{b7} {verb} <t:{}:R>",
        shown_end(end_unix)
    );
    let empty = (players == 0).then(|| NO_POINTS_YET.to_owned());
    let lines = rows.iter().map(|(row, name)| board_line(row, name));
    let footer = rows
        .iter()
        .find(|(row, _)| row.requester)
        .and_then(|(row, name)| requester_footer(row, name, players));
    [title]
        .into_iter()
        .chain(empty)
        .chain(lines)
        .chain(footer)
        .collect::<Vec<_>>()
        .join("\n")
}

fn requester_footer(row: &BoardRow, name: &str, players: usize) -> Option<String> {
    match row.rank {
        None => Some(format!("{name} has no points yet.")),
        Some(rank) if rank > TOP_LINES => Some(format!("{name} is {rank} of {players} players.")),
        Some(_) => None,
    }
}

pub fn no_competition(last: Option<(&str, &[&str], i64)>) -> String {
    match last {
        None => NEVER_RAN.to_owned(),
        Some((name, [], _)) => {
            format!("No IQ Competition is running. The last one, **{name}**, ended with no winner.")
        }
        Some((name, winners, points)) => format!(
            "No IQ Competition is running. The last one, **{name}**, was won by {} with {}.",
            join_names(winners),
            points_text(points)
        ),
    }
}

pub fn winner_announcement(
    competition: &str,
    roles: &[RoleId],
    standings: &[Standing],
    names: &[(UserId, String)],
) -> String {
    let opening = with_mentions(roles, &format!("**{competition}** is over."));
    let top: Vec<&Standing> = standings
        .iter()
        .filter(|standing| standing.rank == 1)
        .collect();
    let mentions: Vec<String> = top.iter().map(|standing| user(standing.user)).collect();
    let mention_refs: Vec<&str> = mentions.iter().map(String::as_str).collect();
    let verdict = match top.as_slice() {
        [] => "Nobody scored, so there is no winner.".to_owned(),
        [only] => format!(
            "The winner is {} with {}.",
            user(only.user),
            points_text(only.points)
        ),
        [first, ..] => format!(
            "It's a tie: {} share the win with {} each.",
            join_names(&mention_refs),
            points_text(first.points)
        ),
    };
    let lines = standings.iter().take(ANNOUNCED_LINES).map(|standing| {
        board_line(
            &BoardRow {
                rank: Some(standing.rank),
                user: standing.user,
                points: standing.points,
                requester: false,
            },
            name_of(standing.user, names),
        )
    });
    [opening, verdict]
        .into_iter()
        .chain(lines)
        .chain(["Thanks to everyone who played, and to the judges.".to_owned()])
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn start_announcement(
    competition: &str,
    end_unix: i64,
    roles: &[RoleId],
    submissions: ChannelId,
    criteria: &[Criterion],
    rules: &str,
    judges: &[UserId],
) -> String {
    let opening = with_mentions(
        roles,
        &format!(
            "**{competition}** has started. It runs until <t:{}:F>.",
            shown_end(end_unix)
        ),
    );
    let how = format!(
        "To score, post in {}: just the criterion name, with exactly 2 screenshots attached. A judge's reaction awards the points. Check where you stand with **/iq leaderboard**.",
        channel(submissions)
    );
    let criteria_lines = criteria.iter().flat_map(|criterion| {
        let heading = format!(
            "**{}** \u{b7} {}",
            criterion.name,
            points_text(i64::from(criterion.points))
        );
        let description = criterion.description.trim();
        [heading]
            .into_iter()
            .chain((!description.is_empty()).then(|| description.to_owned()))
    });
    let rules_lines: Vec<String> = match rules.trim() {
        "" => Vec::new(),
        text => ["## Rules".to_owned()]
            .into_iter()
            .chain(text.lines().map(|line| format!("> {line}")))
            .collect(),
    };
    let judges_line = match judges {
        [] => None,
        some => Some(format!(
            "Judges: {}",
            some.iter()
                .map(|judge| user(*judge))
                .collect::<Vec<_>>()
                .join(" ")
        )),
    };
    [opening, how, "## Criteria".to_owned()]
        .into_iter()
        .chain(criteria_lines)
        .chain(rules_lines)
        .chain(judges_line)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn paused_notice(competition: &str, submissions: ChannelId) -> String {
    format!(
        "**{competition}** is paused. Posts in {} are not checked while it is paused.",
        channel(submissions)
    )
}

pub fn resumed_notice(competition: &str) -> String {
    format!(
        "**{competition}** is back on. Posts made during the pause were not counted; post them again."
    )
}

pub fn edit_note(
    added: &[Criterion],
    changed: &[Criterion],
    removed: &[Criterion],
    rules_changed: bool,
) -> String {
    let with_points = |verb: &str, list: &[Criterion]| {
        (!list.is_empty()).then(|| {
            let names: Vec<String> = list
                .iter()
                .map(|criterion| {
                    format!(
                        "**{}** ({})",
                        criterion.name,
                        points_text(i64::from(criterion.points))
                    )
                })
                .collect();
            format!("{verb} {}", names.join(", "))
        })
    };
    let removed_part = (!removed.is_empty()).then(|| {
        let names: Vec<String> = removed
            .iter()
            .map(|criterion| format!("**{}**", criterion.name))
            .collect();
        format!("removed {}", names.join(", "))
    });
    let parts: Vec<String> = [
        with_points("added", added),
        with_points("changed", changed),
        removed_part,
    ]
    .into_iter()
    .flatten()
    .collect();
    let criteria_line = (!parts.is_empty()).then(|| {
        format!(
            "Criteria updated: {}. The list above is current.",
            parts.join("; ")
        )
    });
    let rules_line =
        rules_changed.then(|| "Rules updated. The rules above are current.".to_owned());
    let lines: Vec<String> = [criteria_line, rules_line].into_iter().flatten().collect();
    match lines.is_empty() {
        true => "Announcement updated. The post above is current.".to_owned(),
        false => lines.join("\n"),
    }
}

pub fn points_log(judge: UserId, member: UserId, points: i64, reason: Option<&str>) -> String {
    let action = match points < 0 {
        true => format!(
            "took {} from {}",
            points_text(points.saturating_abs()),
            user(member)
        ),
        false => format!("gave {} to {}", points_text(points), user(member)),
    };
    let base = format!("{} {action}.", user(judge));
    match given_reason(reason) {
        Some(reason) => format!("{base} Reason: {reason}"),
        None => base,
    }
}

pub fn points_confirmation(
    member: UserId,
    points: i64,
    total: i64,
    reason: Option<&str>,
    submissions: ChannelId,
) -> String {
    let action = match points < 0 {
        true => format!(
            "Took **{}** from {}, who now has {total}.",
            points_text(points.saturating_abs()),
            user(member)
        ),
        false => format!(
            "Gave **{}** to {}, who now has {total}.",
            points_text(points),
            user(member)
        ),
    };
    let reason_line = given_reason(reason).map(|reason| format!("Reason: {reason}"));
    [action]
        .into_iter()
        .chain(reason_line)
        .chain([format!(
            "Logged in {} so players can see the change.",
            channel(submissions)
        )])
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn judges_prompt(competition: &str) -> String {
    format!("**{competition}** judges. Picked members get @IQ-Judge; members you unpick lose it.")
}

pub fn judges_saved(judges: &[UserId]) -> String {
    match judges {
        [] => "Saved. Judges now: nobody.".to_owned(),
        some => format!(
            "Saved. Judges now: {}.",
            some.iter()
                .map(|judge| user(*judge))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

pub fn pause_question(competition: &str, submissions: ChannelId) -> String {
    format!(
        "Pause **{competition}**? Posts in {} are not checked while it is paused. Channels, points and criteria stay. Resume with /iq start.",
        channel(submissions)
    )
}

pub fn paused(announcements: ChannelId) -> String {
    format!(
        "Paused. A notice went to {}, without a ping.",
        channel(announcements)
    )
}

pub fn resume_question(competition: &str, end_unix: i64) -> String {
    format!(
        "**{competition}** is paused. Resume it? It still ends <t:{}:F>.\nTo start a fresh competition instead, run /iq cleanup first.",
        shown_end(end_unix)
    )
}

pub fn already_running(competition: &str) -> String {
    format!(
        "**{competition}** is already running. A server runs one competition at a time. To change it, use /iq edit."
    )
}

pub fn needs_cleanup(competition: &str) -> String {
    format!(
        "**{competition}** is complete. Run /iq cleanup to remove its channels, then /iq start a new one."
    )
}

pub fn complete_too_early(competition: &str, end_unix: i64) -> String {
    format!(
        "**{competition}** ends <t:{}:F>. You can complete it after that. To end it early, pause it with /iq stop.",
        shown_end(end_unix)
    )
}

pub fn complete_question(
    competition: &str,
    end_unix: i64,
    announcements: ChannelId,
    roles: &[RoleId],
    winners: &[&str],
    points: i64,
) -> String {
    let pinging = match roles {
        [] => String::new(),
        some => format!(
            ", pinging {}",
            some.iter()
                .map(|role| role_mention(*role))
                .collect::<Vec<_>>()
                .join(" ")
        ),
    };
    let first = format!(
        "**{competition}** ended on <t:{}:D>. Post the result in {}{pinging}?",
        shown_end(end_unix),
        channel(announcements)
    );
    let bold: Vec<String> = winners.iter().map(|name| format!("**{name}**")).collect();
    let bold_refs: Vec<&str> = bold.iter().map(String::as_str).collect();
    let second = match bold_refs.as_slice() {
        [] => "Nobody scored, so there is no winner.".to_owned(),
        [only] => format!("Winner: {only} with {}.", points_text(points)),
        many => format!(
            "Winners: {} with {} each.",
            join_names(many),
            points_text(points)
        ),
    };
    format!("{first}\n{second}")
}

pub fn cleanup_question(competition: &str) -> String {
    format!(
        "Delete the **{competition}** category and both of its channels? Their messages can't be recovered.\nKept: the final scores (archived), the criteria and rules (saved as a template). The next /iq start begins at zero."
    )
}

pub fn cleaned_up(competition: &str, templates: usize) -> String {
    match templates {
        0 | 1 => {
            format!("Cleaned up. **{competition}** is archived and is now your only template.")
        }
        count => format!(
            "Cleaned up. **{competition}** is archived and is now the newest of your {count} templates."
        ),
    }
}

pub fn cleanup_while_running(competition: &str) -> String {
    format!(
        "**{competition}** is still running. Pause it with /iq stop or finish it with /iq complete, then clean up."
    )
}

pub fn name_of(member: UserId, names: &[(UserId, String)]) -> &str {
    names
        .iter()
        .find(|(user, _)| *user == member)
        .map(|(_, name)| name.as_str())
        .unwrap_or(UNKNOWN_MEMBER)
}

fn given_reason(reason: Option<&str>) -> Option<&str> {
    reason.map(str::trim).filter(|reason| !reason.is_empty())
}

fn shown_end(end_unix: i64) -> i64 {
    end_unix - SHOWN_END_OFFSET
}

fn points_text(points: i64) -> String {
    match points {
        1 => "1 point".to_owned(),
        other => format!("{other} points"),
    }
}

fn with_mentions(roles: &[RoleId], text: &str) -> String {
    match roles {
        [] => text.to_owned(),
        some => format!(
            "{} {text}",
            some.iter()
                .map(|role| role_mention(*role))
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
}

fn role_mention(role: RoleId) -> String {
    format!("<@&{role}>")
}

fn user(member: UserId) -> String {
    format!("<@{}>", member.get())
}

fn channel(id: ChannelId) -> String {
    format!("<#{}>", id.get())
}

pub const SETUP_STEPS: usize = 5;
pub const TEMPLATE_PLACEHOLDER: &str = "Start from";
pub const BLANK_LABEL: &str = "Start blank";
pub const BLANK_DESCRIPTION: &str = "No criteria, rules or roles filled in";
pub const FIRST_DAY_PLACEHOLDER: &str = "First day";
pub const LAST_DAY_PLACEHOLDER: &str = "Last day";
pub const ZONE_PLACEHOLDER: &str = "Time zone for the end";
pub const CRITERION_PLACEHOLDER: &str = "Pick a criterion to edit or remove";
pub const ROLES_PLACEHOLDER: &str = "Roles that can take part (pick any)";
pub const JUDGES_PLACEHOLDER: &str = "Members who judge (pick any)";
pub const REACTION_PLACEHOLDER: &str = "Judging reaction";
pub const RENAME_LABEL: &str = "Rename";
pub const NEXT_CRITERIA_LABEL: &str = "Next: criteria";
pub const NEXT_RULES_LABEL: &str = "Next: rules";
pub const NEXT_WHO_LABEL: &str = "Next: who";
pub const NEXT_REVIEW_LABEL: &str = "Next: review";
pub const ADD_CRITERION_LABEL: &str = "Add criterion";
pub const EDIT_PICKED_LABEL: &str = "Edit picked";
pub const REMOVE_PICKED_LABEL: &str = "Remove picked";
pub const WRITE_RULES_LABEL: &str = "Write rules";
pub const START_LABEL: &str = "Start competition";
pub const SAVE_CHANGES_LABEL: &str = "Save changes";
pub const BACK_LABEL: &str = "Back";
pub const CANCEL_LABEL: &str = "Cancel";
pub const BASICS_PROMPT: &str = "Pick where to start from and the dates it runs.";
pub const NO_TEMPLATES: &str =
    "No past competitions yet, so there are no templates. The last 10 are kept once you have some.";
pub const TEMPLATES_OFFERED: &str = "Templates are the last 10 competitions. Picking one copies its criteria, rules, roles, judges and reaction.";
pub const CRITERIA_INTRO: &str =
    "Each criterion is something a player posts screenshots for. The name is what they type.";
pub const NO_CRITERIA: &str = "No criteria yet. Add at least one.";
pub const RULES_INTRO: &str =
    "The rules are posted in the read-only announcements channel when the competition starts.";
pub const NO_RULES: &str = "No rules written yet.";
pub const WHO_ROLES_INTRO: &str = "Only members with a picked role can see the competition's channels, and the start announcement pings those roles.";
pub const WHO_JUDGES_INTRO: &str =
    "Judges get the @IQ-Judge role. Only their reaction awards points.";
pub const STARTING_CREATES: &str = "Starting creates:";
pub const SUBMISSIONS_BRANCH: &str = "> #iq-submissions \u{b7} players post here";
pub const ROLE_BELOW_JUDGE: &str = "I can't hand out @IQ-Judge: my role sits below it. Drag my role above @IQ-Judge in Server Settings, then press Start again.";
pub const MISSING_SETUP_PERMISSIONS: &str =
    "I need Manage Channels and Manage Roles in this server to set up the competition.";
pub const RENAME_MODAL_TITLE: &str = "Competition name";
pub const RENAME_INPUT_LABEL: &str = "Name, also used for the category";
pub const ADD_CRITERION_TITLE: &str = "Add criterion";
pub const EDIT_CRITERION_TITLE: &str = "Edit criterion";
pub const CRITERION_NAME_LABEL: &str = "Name players type";
pub const CRITERION_DESCRIPTION_LABEL: &str = "What earns it";
pub const CRITERION_POINTS_LABEL: &str = "Points, a whole number";
pub const RULES_MODAL_TITLE: &str = "Rules";
pub const RULES_INPUT_LABEL: &str = "Rules, up to 4000 characters";
pub const ZONE_MODAL_TITLE: &str = crate::text::ZONE_MODAL_TITLE;
pub const ZONE_INPUT_LABEL: &str = crate::text::ZONE_INPUT_LABEL;

pub fn setup_heading(name: &str, editing: bool) -> String {
    match editing {
        true => format!("**Editing {}**", escape(name)),
        false => format!("**New IQ Competition: {}**", escape(name)),
    }
}

pub fn setup_step_line(step: SetupStep) -> String {
    format!(
        "**Step {} of {SETUP_STEPS}:** {}",
        step_number(step),
        step_name(step)
    )
}

fn step_number(step: SetupStep) -> usize {
    match step {
        SetupStep::Basics => 1,
        SetupStep::Criteria => 2,
        SetupStep::Rules => 3,
        SetupStep::Who => 4,
        SetupStep::Review => 5,
    }
}

fn step_name(step: SetupStep) -> &'static str {
    match step {
        SetupStep::Basics => "Basics",
        SetupStep::Criteria => "Criteria",
        SetupStep::Rules => "Rules",
        SetupStep::Who => "Who",
        SetupStep::Review => "Review",
    }
}

pub fn competition_name(first_day: Date) -> String {
    format!("{} IQ", first_day.strftime("%B"))
}

pub fn basics_line(first_day: Date, last_day: Date, zone: &str) -> String {
    format!(
        "Runs {} to {}, ending at 23:59 {zone}.",
        day_label(first_day),
        day_label(last_day)
    )
}

pub fn template_copied(template: &str) -> String {
    format!(
        "Criteria, rules, roles, judges and reaction copied from {}. Change any of them in the next steps.",
        escape(template)
    )
}

pub fn template_description(first_day: Date, last_day: Date, criteria: usize) -> String {
    let span = if first_day.year() == last_day.year() && first_day.month() == last_day.month() {
        format!(
            "{} to {}",
            first_day.strftime("%-d"),
            last_day.strftime("%-d %b")
        )
    } else {
        format!(
            "{} to {}",
            first_day.strftime("%-d %b"),
            last_day.strftime("%-d %b")
        )
    };
    format!("Ran {span} \u{b7} {}", criteria_count(criteria))
}

pub fn criterion_line(number: usize, criterion: &Criterion) -> String {
    let heading = format!(
        "{number}. **{}** \u{b7} {}",
        escape(&criterion.name),
        points_text(i64::from(criterion.points))
    );
    match criterion.description.is_empty() {
        true => heading,
        false => format!("{heading}\n{}", escape(&criterion.description)),
    }
}

pub fn criterion_option_description(criterion: &Criterion) -> String {
    points_text(i64::from(criterion.points))
}

pub fn criteria_not_shown(count: usize) -> String {
    match count {
        1 => "1 more criterion not shown.".to_owned(),
        more => format!("{more} more criteria not shown."),
    }
}

pub fn rules_not_shown(characters: usize) -> String {
    match characters {
        1 => "1 more character not shown.".to_owned(),
        more => format!("{more} more characters not shown."),
    }
}

pub fn reaction_label(reaction: JudgingReaction) -> String {
    format!("{} {}", reaction.emoji(), reaction.value())
}

pub fn review_runs(first_day: Date, last_day: Date, zone: &str, criteria: usize) -> String {
    format!(
        "Runs {} to {}, ending at 23:59 {zone}. {}.",
        day_label(first_day),
        day_label(last_day),
        criteria_count(criteria)
    )
}

pub fn review_who(roles: &str, judges: &str, reaction: JudgingReaction) -> String {
    format!(
        "Visible to {roles}. Judges: {judges}. Judging reaction: {}",
        reaction_label(reaction)
    )
}

pub fn role_list(roles: &[RoleId]) -> String {
    listed(roles.iter().map(|role| role_mention(*role)).collect())
}

pub fn judge_list(judges: &[UserId]) -> String {
    listed(judges.iter().map(|judge| user(*judge)).collect())
}

pub fn role_count(count: usize) -> String {
    match count {
        1 => "1 role".to_owned(),
        more => format!("{more} roles"),
    }
}

pub fn judge_count(count: usize) -> String {
    match count {
        1 => "1 judge".to_owned(),
        more => format!("{more} judges"),
    }
}

pub fn category_branch(name: &str) -> String {
    format!("> **{}**", escape(name))
}

pub fn announcements_branch(roles: &str) -> String {
    format!("> #iq-announcements \u{b7} read-only, rules and criteria, pings {roles}")
}

pub fn setup_refusal(refusal: &SetupRefusal) -> String {
    match refusal {
        SetupRefusal::CriterionNameTaken(name) => {
            format!(
                "A criterion named {name} already exists. Names ignore capitals, so pick another."
            )
        }
        SetupRefusal::CriterionPoints => "Points must be a whole number from 1 to 1000.".to_owned(),
        SetupRefusal::TooManyCriteria => "A competition holds at most 25 criteria.".to_owned(),
        SetupRefusal::CriterionName => "A criterion needs a name of 1 to 50 characters.".to_owned(),
        SetupRefusal::CriterionDescription => {
            "A criterion's description holds at most 300 characters.".to_owned()
        }
        SetupRefusal::CriterionUnknown => "Pick a criterion from the menu first.".to_owned(),
        SetupRefusal::NameFormat => "A competition name needs 1 to 60 characters.".to_owned(),
        SetupRefusal::RulesTooLong => "The rules hold at most 4000 characters.".to_owned(),
        SetupRefusal::TooManyRoles => "Pick at most 25 roles.".to_owned(),
        SetupRefusal::TooManyJudges => "Pick at most 25 judges.".to_owned(),
        SetupRefusal::TemplateUnknown => {
            "That template is no longer offered. Pick one from the menu.".to_owned()
        }
        SetupRefusal::DayNotOffered => "Pick a day from the menu.".to_owned(),
        SetupRefusal::LastDayNotAfterFirst => {
            "The last day must come after the first day.".to_owned()
        }
        SetupRefusal::ZoneUnknown => {
            "I don't know that time zone. Use a name like America/Edmonton.".to_owned()
        }
        SetupRefusal::ReactionUnknown => "Pick a judging reaction from the menu.".to_owned(),
        SetupRefusal::FixedAfterStart => {
            "A started competition keeps its template, name, dates and time zone.".to_owned()
        }
        SetupRefusal::StepIncomplete(step) => step_need(*step).to_owned(),
    }
}

fn step_need(step: SetupStep) -> &'static str {
    match step {
        SetupStep::Basics => "Pick where to start from, the first day and the last day first.",
        SetupStep::Criteria => "Add at least one criterion first.",
        SetupStep::Rules => "Write the rules first.",
        SetupStep::Who => "Pick at least one role and one judge first.",
        SetupStep::Review => "Check the review, then press Start.",
    }
}

fn criteria_count(count: usize) -> String {
    match count {
        1 => "1 criterion".to_owned(),
        more => format!("{more} criteria"),
    }
}

fn listed(names: Vec<String>) -> String {
    match names.is_empty() {
        true => "nobody".to_owned(),
        false => names.join(", "),
    }
}

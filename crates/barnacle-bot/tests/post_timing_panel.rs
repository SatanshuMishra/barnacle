use barnacle_bot::attendance_store::Event;
use barnacle_bot::attendance_store::EventChange;
use barnacle_bot::event_draft;
use barnacle_bot::event_draft::CreateOptions;
use barnacle_bot::event_draft::Draft;
use barnacle_bot::event_draft::Drafts;
use barnacle_bot::event_draft::EditOptions;
use barnacle_bot::event_draft::PanelView;
use barnacle_bot::event_draft::Ready;
use barnacle_bot::event_draft::Step;
use barnacle_bot::ids::ChannelId;
use barnacle_bot::ids::GuildId;
use barnacle_bot::schedule::CleanUpDelay;
use barnacle_bot::schedule::PostLead;
use barnacle_bot::schedule::Schedule;
use barnacle_bot::schedule::Span;
use barnacle_bot::schedule::Zone;
use barnacle_bot::text::Refusal;
use barnacle_bot::wiring;
use barnacle_bot::wiring::PanelPart;
use barnacle_guess::UserId;
use jiff::civil::date;

const SERVER: GuildId = GuildId::new(1);
const CHANNEL: ChannelId = ChannelId::new(10);
const ADMIN: UserId = UserId::new(100);
const PANEL: u64 = 7;
const NOW: i64 = 1_793_569_140;
const NOW_MS: u64 = 1_793_569_140_000;
const POST_LEAD_LABELS: [&str; 7] = [
    "Post sign-ups 1 day before each session (default)",
    "Post sign-ups 2 days before each session",
    "Post sign-ups 3 days before each session",
    "Post sign-ups 4 days before each session",
    "Post sign-ups 5 days before each session",
    "Post sign-ups 6 days before each session",
    "Post sign-ups 7 days before each session",
];
const CLEAN_UP_LABELS: [&str; 9] = [
    "Remove sign-ups as soon as the session ends",
    "Remove sign-ups 30 minutes after the session ends (default)",
    "Remove sign-ups 1 hour after the session ends",
    "Remove sign-ups 1 hour 30 minutes after the session ends",
    "Remove sign-ups 2 hours after the session ends",
    "Remove sign-ups 2 hours 30 minutes after the session ends",
    "Remove sign-ups 3 hours after the session ends",
    "Remove sign-ups 3 hours 30 minutes after the session ends",
    "Remove sign-ups 4 hours after the session ends",
];

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn fresh() -> Draft {
    Draft::create(
        &CreateOptions {
            name: "Scrim Nights".to_owned(),
            template: None,
            zone: Zone::find("America/Edmonton"),
        },
        SERVER,
        CHANNEL,
        ADMIN,
        NOW,
    )
    .unwrap()
}

fn advanced_to_pings(dated: Draft) -> Draft {
    let pings = dated
        .with_weekdays(&strings(&["mon", "wed", "fri"]))
        .unwrap()
        .next()
        .unwrap()
        .with_typed_time("14:30", "18:30")
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(pings.step(), Step::Pings);
    pings
}

fn pings_from(first: &str, last: &str) -> Draft {
    advanced_to_pings(fresh().with_typed_dates(first, last, NOW).unwrap())
}

fn at_pings() -> Draft {
    pings_from("2026-11-02", "2026-11-06")
}

fn rows(view: &PanelView) -> Vec<serde_json::Value> {
    view.components
        .iter()
        .map(|row| serde_json::to_value(row).unwrap())
        .collect()
}

fn menu(view: &PanelView, part: PanelPart) -> serde_json::Value {
    let id = wiring::panel_id(PANEL, part);
    rows(view)
        .into_iter()
        .flat_map(|row| row["components"].as_array().cloned().unwrap_or_default())
        .find(|component| component["custom_id"] == id.as_str())
        .unwrap_or_else(|| panic!("the panel has no component {id}"))
}

fn option_texts(view: &PanelView, part: PanelPart, key: &str) -> Vec<String> {
    menu(view, part)["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|option| option[key].as_str().unwrap().to_owned())
        .collect()
}

fn defaults(view: &PanelView, part: PanelPart) -> Vec<String> {
    menu(view, part)["options"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|option| option["default"] == true)
        .map(|option| option["value"].as_str().unwrap().to_owned())
        .collect()
}

fn lines(view: &PanelView) -> Vec<String> {
    view.content.lines().map(str::to_owned).collect()
}

fn ranked_finals() -> Event {
    Event {
        id: 3,
        guild: SERVER,
        channel: CHANNEL,
        name: "Ranked Finals".to_owned(),
        span: Span::new(
            date(2026, 11, 2),
            date(2026, 11, 20),
            Schedule::clan_battles()
                .with_post_lead(PostLead::from_days(4).unwrap())
                .with_clean_up(CleanUpDelay::from_minutes(180).unwrap()),
        )
        .unwrap(),
        created_by: ADMIN,
        created_at_ms: 1_791_800_000_000,
        ping_roles: Vec::new(),
        ended_at_ms: None,
        posts_up: false,
    }
}

fn editing_at_pings() -> Draft {
    Draft::edit(&ranked_finals(), &EditOptions::default(), ADMIN, NOW)
        .unwrap()
        .next()
        .unwrap()
        .next()
        .unwrap()
}

fn created_timing(draft: &Draft) -> (u8, u16) {
    match draft.ready(NOW, &[]) {
        Ok(Ready::Create { new, .. }) => {
            let schedule = new.span.schedule();
            (schedule.post_lead().days(), schedule.clean_up().minutes())
        }
        other => panic!("the draft was not ready to create: {other:?}"),
    }
}

#[test]
fn the_pings_step_offers_seven_post_leads_with_one_day_picked() {
    let view = at_pings().panel(PANEL, NOW);
    assert_eq!(
        option_texts(&view, PanelPart::PostLead, "value"),
        ["1", "2", "3", "4", "5", "6", "7"]
    );
    assert_eq!(
        option_texts(&view, PanelPart::PostLead, "label"),
        POST_LEAD_LABELS
    );
    assert_eq!(defaults(&view, PanelPart::PostLead), ["1"]);
    let lead = menu(&view, PanelPart::PostLead);
    assert_eq!(lead["custom_id"], "barnacle-event:7:post-lead");
    assert_eq!(lead["placeholder"], "When sign-up posts go up");
    assert_eq!(lead["min_values"], 1);
    assert_eq!(lead["max_values"], 1);
}

#[test]
fn the_pings_step_offers_nine_clean_up_delays_with_thirty_minutes_picked() {
    let view = at_pings().panel(PANEL, NOW);
    assert_eq!(
        option_texts(&view, PanelPart::CleanUp, "value"),
        ["0", "30", "60", "90", "120", "150", "180", "210", "240"]
    );
    assert_eq!(
        option_texts(&view, PanelPart::CleanUp, "label"),
        CLEAN_UP_LABELS
    );
    assert_eq!(defaults(&view, PanelPart::CleanUp), ["30"]);
    let clean_up = menu(&view, PanelPart::CleanUp);
    assert_eq!(clean_up["custom_id"], "barnacle-event:7:clean-up");
    assert_eq!(clean_up["placeholder"], "When sign-up posts are removed");
    assert_eq!(clean_up["min_values"], 1);
    assert_eq!(clean_up["max_values"], 1);
}

#[test]
fn the_pings_step_puts_the_timing_menus_between_roles_and_buttons() {
    let rows = rows(&at_pings().panel(PANEL, NOW));
    assert_eq!(rows.len(), 4);
    let first_ids: Vec<&str> = rows[..3]
        .iter()
        .map(|row| row["components"][0]["custom_id"].as_str().unwrap())
        .collect();
    assert_eq!(
        first_ids,
        [
            "barnacle-event:7:roles",
            "barnacle-event:7:post-lead",
            "barnacle-event:7:clean-up"
        ]
    );
    let buttons: Vec<&str> = rows[3]["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|button| button["custom_id"].as_str().unwrap())
        .collect();
    assert_eq!(
        buttons,
        [
            "barnacle-event:7:back",
            "barnacle-event:7:save",
            "barnacle-event:7:everyone",
            "barnacle-event:7:cancel"
        ]
    );
}

#[test]
fn the_pings_step_states_both_choices_in_words() {
    let draft = at_pings();
    assert!(lines(&draft.panel(PANEL, NOW)).contains(
        &"Sign-up posts go up 1 day before each session and are removed 30 minutes after it ends."
            .to_owned()
    ));
    let picked = draft
        .with_post_lead(&strings(&["2"]))
        .unwrap()
        .with_clean_up(&strings(&["0"]))
        .unwrap();
    assert!(
        lines(&picked.panel(PANEL, NOW)).contains(
            &"Sign-up posts go up 2 days before each session and are removed as soon as it ends."
                .to_owned()
        )
    );
    let time_step = draft.back();
    let days_step = time_step.back();
    for earlier in [time_step, days_step] {
        assert!(
            !lines(&earlier.panel(PANEL, NOW))
                .iter()
                .any(|line| line.starts_with("Sign-up posts go up")),
            "{:?}",
            earlier.step()
        );
    }
}

#[test]
fn picking_a_timing_redraws_it_as_picked() {
    let drafts = Drafts::without_expiry();
    drafts.open(PANEL, at_pings(), NOW_MS);
    let after_lead =
        event_draft::post_lead_picked(&drafts, PANEL, ADMIN, &strings(&["3"]), NOW_MS).unwrap();
    assert_eq!(defaults(&after_lead, PanelPart::PostLead), ["3"]);
    let after_clean_up =
        event_draft::clean_up_picked(&drafts, PANEL, ADMIN, &strings(&["90"]), NOW_MS).unwrap();
    assert_eq!(defaults(&after_clean_up, PanelPart::CleanUp), ["90"]);
    assert_eq!(defaults(&after_clean_up, PanelPart::PostLead), ["3"]);
    let kept = drafts.with(PANEL, ADMIN, NOW_MS).unwrap();
    assert_eq!(kept.post_lead().days(), 3);
    assert_eq!(kept.clean_up().minutes(), 90);
}

#[test]
fn create_carries_the_picked_timing_into_the_new_event() {
    let picked = at_pings()
        .with_post_lead(&strings(&["5"]))
        .unwrap()
        .with_clean_up(&strings(&["120"]))
        .unwrap();
    assert_eq!(created_timing(&picked), (5, 120));
}

#[test]
fn create_without_a_pick_saves_today_s_timing() {
    assert_eq!(created_timing(&at_pings()), (1, 30));
}

#[test]
fn an_offered_timing_is_taken() {
    let draft = at_pings();
    assert_eq!(
        draft
            .with_post_lead(&strings(&["7"]))
            .map(|picked| picked.post_lead().days()),
        Ok(7)
    );
    assert_eq!(
        draft
            .with_clean_up(&strings(&["240"]))
            .map(|picked| picked.clean_up().minutes()),
        Ok(240)
    );
}

#[test]
fn a_timing_number_off_the_menu_is_refused() {
    let draft = at_pings();
    for lead in ["0", "8"] {
        assert_eq!(
            draft.with_post_lead(&strings(&[lead])),
            Err(Refusal::PostTimingInvalid),
            "{lead}"
        );
    }
    for delay in ["45", "270"] {
        assert_eq!(
            draft.with_clean_up(&strings(&[delay])),
            Err(Refusal::PostTimingInvalid),
            "{delay}"
        );
    }
}

#[test]
fn a_timing_that_is_not_a_whole_number_is_refused() {
    let draft = at_pings();
    for lead in ["two", "2.5"] {
        assert_eq!(
            draft.with_post_lead(&strings(&[lead])),
            Err(Refusal::PostTimingInvalid),
            "{lead}"
        );
    }
    for delay in ["half", "-30"] {
        assert_eq!(
            draft.with_clean_up(&strings(&[delay])),
            Err(Refusal::PostTimingInvalid),
            "{delay}"
        );
    }
}

#[test]
fn a_timing_pick_needs_exactly_one_value() {
    let draft = at_pings();
    for leads in [&[][..], &["2", "3"][..]] {
        assert_eq!(
            draft.with_post_lead(&strings(leads)),
            Err(Refusal::PostTimingInvalid),
            "{leads:?}"
        );
    }
    for delays in [&[][..], &["0", "30"][..]] {
        assert_eq!(
            draft.with_clean_up(&strings(delays)),
            Err(Refusal::PostTimingInvalid),
            "{delays:?}"
        );
    }
}

#[test]
fn the_timing_refusal_says_how_to_fix_it() {
    assert_eq!(Refusal::PostTimingInvalid.code(), "post_timing_invalid");
    assert_eq!(
        Refusal::PostTimingInvalid.message(),
        "Pick one choice from the menu."
    );
}

#[test]
fn a_typed_first_day_in_the_past_is_kept() {
    let draft = fresh()
        .with_typed_dates("2026-10-26", "2026-11-06", NOW)
        .unwrap();
    assert_eq!(draft.first_day(), Some(date(2026, 10, 26)));
    assert_eq!(draft.last_day(), Some(date(2026, 11, 6)));
}

#[test]
fn an_event_that_began_in_the_past_saves() {
    match pings_from("2026-10-26", "2026-11-06").ready(NOW, &[]) {
        Ok(Ready::Create { new, .. }) => {
            assert_eq!(new.span.first_day(), date(2026, 10, 26));
            assert_eq!(new.span.sessions_left(NOW), 3);
        }
        other => panic!("the event did not save: {other:?}"),
    }
}

#[test]
fn an_event_with_every_session_started_is_still_refused() {
    let dated = fresh().with_typed_dates("2026-10-19", "2026-10-30", NOW);
    assert!(dated.is_ok());
    assert_eq!(
        advanced_to_pings(dated.unwrap()).ready(NOW, &[]),
        Err(Refusal::NoSessionsLeft)
    );
}

#[test]
fn the_edit_panel_preselects_the_stored_timing() {
    let view = editing_at_pings().panel(PANEL, NOW);
    assert_eq!(defaults(&view, PanelPart::PostLead), ["4"]);
    assert_eq!(defaults(&view, PanelPart::CleanUp), ["180"]);
}

#[test]
fn picking_the_stored_timing_again_changes_nothing() {
    let picked = editing_at_pings()
        .with_post_lead(&strings(&["4"]))
        .unwrap()
        .with_clean_up(&strings(&["180"]))
        .unwrap();
    assert_eq!(picked.ready(NOW, &[]), Err(Refusal::NothingToChange));
}

#[test]
fn a_new_timing_is_the_only_change_saved() {
    let picked = editing_at_pings().with_clean_up(&strings(&["0"])).unwrap();
    assert_eq!(
        picked.ready(NOW, &[]),
        Ok(Ready::Edit {
            event: 3,
            created_at_ms: 1_791_800_000_000,
            change: EventChange {
                clean_up: Some(CleanUpDelay::from_minutes(0).unwrap()),
                ..EventChange::default()
            },
            warning: None,
        })
    );
}

#[test]
fn the_timing_menus_have_their_own_panel_ids() {
    assert_eq!(
        wiring::panel_id(12, PanelPart::PostLead),
        "barnacle-event:12:post-lead"
    );
    assert_eq!(
        wiring::panel_id(12, PanelPart::CleanUp),
        "barnacle-event:12:clean-up"
    );
    assert_eq!(
        wiring::panel_part("barnacle-event:12:post-lead"),
        Some((12, PanelPart::PostLead))
    );
    assert_eq!(
        wiring::panel_part("barnacle-event:12:clean-up"),
        Some((12, PanelPart::CleanUp))
    );
}

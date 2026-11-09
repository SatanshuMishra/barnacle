use barnacle_bot::ids::GuildId;
use barnacle_bot::iq_setup;
use barnacle_bot::iq_setup::SetupDraft;
use barnacle_bot::schedule::Zone;
use barnacle_guess::UserId;
use jiff::civil::date;

const OWNER: UserId = UserId::new(100);
const OPENED_AT_MS: u64 = 1000;
const THIRTY_DAYS_MS: u64 = 30 * 24 * 60 * 60 * 1000;

fn draft(name: &str) -> SetupDraft {
    SetupDraft::new(
        GuildId::new(1),
        OWNER,
        vec![],
        Zone::find("Europe/Berlin").unwrap(),
        date(2026, 11, 2),
    )
    .renamed(name)
    .unwrap()
}

#[test]
fn an_iq_panel_still_works_thirty_days_later() {
    let drafts = iq_setup::drafts();
    let opened = draft("November IQ");
    drafts.open(7, opened.clone(), OPENED_AT_MS);
    let later = OPENED_AT_MS + THIRTY_DAYS_MS;
    assert_eq!(drafts.with(7, OWNER, later), Ok(opened.clone()));
    assert_eq!(drafts.take(7, OWNER, later), Ok(opened));
}

#[test]
fn opening_a_panel_keeps_an_old_iq_panel() {
    let drafts = iq_setup::drafts();
    let first = draft("November IQ");
    let second = draft("December IQ");
    let later = OPENED_AT_MS + THIRTY_DAYS_MS;
    drafts.open(1, first.clone(), OPENED_AT_MS);
    drafts.open(2, second, later);
    assert_eq!(drafts.with(1, OWNER, later), Ok(first));
}

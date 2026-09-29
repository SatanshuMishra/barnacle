use barnacle_bot::release_notes::Change;
use barnacle_bot::release_notes::FOOTER;
use barnacle_bot::release_notes::LineFault;
use barnacle_bot::release_notes::NoteProblem;
use barnacle_bot::release_notes::NotesError;
use barnacle_bot::release_notes::Release;
use barnacle_bot::release_notes::TitleFault;
use barnacle_bot::release_notes::all;
use barnacle_bot::release_notes::find;
use barnacle_bot::release_notes::parse;
use barnacle_bot::release_notes::problems;

const SOUND: &str = "Fixed a slow reply.";
const SUMMARY: &str = "Small fixes for a smoother game.";
const STORY: &str = "You can now skip a round.";

fn fixes(version: &str, lines: &[&str]) -> String {
    let quoted: Vec<String> = lines.iter().map(|line| format!("'{line}'")).collect();
    format!(
        "[[release]]\nversion = \"{version}\"\nsummary = \"{SUMMARY}\"\nfixes = [{}]\n",
        quoted.join(", ")
    )
}

fn feature(version: &str, body: &str) -> String {
    format!(
        "[[release]]\nversion = \"{version}\"\nsummary = \"{SUMMARY}\"\n\n[[release.feature]]\n{body}"
    )
}

fn reported(text: &str) -> Result<Vec<NoteProblem>, NotesError> {
    Ok(problems(&parse(text)?))
}

fn line(version: &str, text: &str, fault: LineFault) -> NoteProblem {
    NoteProblem::Line {
        version: version.to_owned(),
        line: text.to_owned(),
        fault,
    }
}

fn names_its_version_and_line(problems: &[NoteProblem], version: &str) {
    assert!(!problems.is_empty());
    for problem in problems {
        let shown = problem.to_string();
        assert!(shown.contains(version), "{shown}");
        if let NoteProblem::Line { line, .. } = problem {
            assert!(shown.contains(line.as_str()), "{shown}");
        }
    }
}

#[test]
fn the_running_version_has_notes() -> Result<(), NotesError> {
    assert!(find(&all()?, env!("CARGO_PKG_VERSION")).is_some());
    Ok(())
}

#[test]
fn the_notes_file_breaks_no_plain_language_rule() -> Result<(), NotesError> {
    let found: Vec<String> = problems(&all()?).iter().map(ToString::to_string).collect();
    assert_eq!(found, Vec::<String>::new());
    Ok(())
}

#[test]
fn a_technical_or_malformed_note_is_reported() -> Result<(), NotesError> {
    let longest = format!("{}.", "a".repeat(119));
    let too_long = format!("{}.", "a".repeat(120));
    let cases = [
        (
            "0.3.0",
            format!("{}{}", fixes("0.3.0", &[SOUND]), fixes("0.3.0", &[SOUND])),
            vec![NoteProblem::DuplicateVersion {
                version: "0.3.0".to_owned(),
            }],
        ),
        (
            "0.2",
            fixes("0.2", &[SOUND]),
            vec![NoteProblem::BadVersion {
                version: "0.2".to_owned(),
            }],
        ),
        (
            "0.4.0",
            format!("[[release]]\nversion = \"0.4.0\"\nsummary = \"{SUMMARY}\"\n"),
            vec![NoteProblem::NoContent {
                version: "0.4.0".to_owned(),
            }],
        ),
        (
            "0.4.1",
            format!("[[release]]\nversion = \"0.4.1\"\nfixes = ['{SOUND}']\n"),
            vec![NoteProblem::MissingSummary {
                version: "0.4.1".to_owned(),
            }],
        ),
        (
            "0.5.0",
            feature(
                "0.5.0",
                &format!("title = \"Skip a round\"\nstory = [\"{STORY}\"]\n"),
            ),
            vec![NoteProblem::MissingSteps {
                version: "0.5.0".to_owned(),
                title: "Skip a round".to_owned(),
            }],
        ),
        (
            "0.6.0",
            feature(
                "0.6.0",
                "title = \"Skip a round\"\nsteps = [\"Press Skip.\"]\n",
            ),
            vec![NoteProblem::MissingStory {
                version: "0.6.0".to_owned(),
                title: "Skip a round".to_owned(),
            }],
        ),
        (
            "0.6.1",
            feature(
                "0.6.1",
                &format!(
                    "title = \"Skip a round\"\nstory = [\"{STORY}\", \"{STORY}\", \"{STORY}\"]\nsteps = [\"Press Skip.\"]\n"
                ),
            ),
            vec![NoteProblem::LongStory {
                version: "0.6.1".to_owned(),
                title: "Skip a round".to_owned(),
                sentences: 3,
            }],
        ),
        (
            "0.7.0",
            fixes("0.7.0", &[&too_long]),
            vec![line("0.7.0", &too_long, LineFault::TooLong { length: 121 })],
        ),
        (
            "0.8.0",
            fixes("0.8.0", &["Fixed a slow reply"]),
            vec![line(
                "0.8.0",
                "Fixed a slow reply",
                LineFault::NoFinalPunctuation,
            )],
        ),
        (
            "0.9.0",
            fixes("0.9.0", &["Fixed it. Also more."]),
            vec![line(
                "0.9.0",
                "Fixed it. Also more.",
                LineFault::SentenceBreak,
            )],
        ),
        (
            "0.10.0",
            fixes("0.10.0", &["Fixed the `Table::start` race."]),
            vec![
                line(
                    "0.10.0",
                    "Fixed the `Table::start` race.",
                    LineFault::Technical { sequence: "`" },
                ),
                line(
                    "0.10.0",
                    "Fixed the `Table::start` race.",
                    LineFault::Technical { sequence: "::" },
                ),
            ],
        ),
        (
            "0.11.0",
            fixes("0.11.0", &[""]),
            vec![line("0.11.0", "", LineFault::Empty)],
        ),
    ];
    for (version, text, expected) in cases {
        let found = reported(&text)?;
        assert_eq!(found, expected, "{version}");
        names_its_version_and_line(&found, version);
    }
    for sequence in [
        "`", "**", "__", "::", "->", "http", ".rs", ".toml", ".sql", "{", "}",
    ] {
        let text = format!("Fixed the {sequence} thing.");
        assert_eq!(
            reported(&fixes("0.12.0", &[&text]))?,
            vec![line("0.12.0", &text, LineFault::Technical { sequence })]
        );
    }
    assert_eq!(reported(&fixes("0.13.0", &[&longest]))?, Vec::new());
    let crowded = vec![longest.as_str(); 60];
    let found = reported(&fixes("0.14.0", &crowded))?;
    assert!(found.iter().any(
        |problem| matches!(problem, NoteProblem::PostTooLong { version, .. } if version == "0.14.0")
    ));
    Ok(())
}

#[test]
fn a_release_renders_as_a_styled_message() -> Result<(), NotesError> {
    let releases = all()?;
    let release = find(&releases, "0.2.0").unwrap();
    assert_eq!(
        release.render(),
        "# Barnacle Update 0.2.0\n\
         This update lets you play several silhouette rounds in a row and lets servers choose where Barnacle posts news.\n\
         \n\
         ## New features\n\
         \n\
         ### Silhouette series\n\
         Until now, each round of the ship silhouette game (/guess) had to be started by hand, one at a time. A series plays up to 20 rounds back to back and keeps score for everyone taking part.\n\
         **How to use it**\n\
         1. Type /guess-series in any channel where /guess works.\n\
         2. Choose how many rounds to play, from 2 to 20; if you leave it out, you get 10.\n\
         3. Name each ship in chat as it appears, just like /guess.\n\
         **What to expect**\n\
         - A 5-second countdown appears before every ship, so you are ready to answer.\n\
         - After each round, the standings show who has won the most rounds and who was fastest so far.\n\
         - The standings clear away when the next ship appears, so the channel stays tidy.\n\
         - If nobody talks for three rounds, Barnacle asks if you are still playing; press Yes within 10 seconds to keep going.\n\
         - When the series ends, the final standings stay in the channel for everyone to see.\n\
         \n\
         ### Update news channel\n\
         -# For server managers\n\
         Barnacle can now post news like this in a channel your server picks, so members hear about changes as they happen.\n\
         **How to use it**\n\
         1. Someone with Manage Server types /updates setup.\n\
         2. They pick the channel for the news and, if they like, a role to notify.\n\
         \n\
         -# Questions or feedback? Reach out to a server administrator."
    );
    assert_eq!(
        FOOTER,
        "Questions or feedback? Reach out to a server administrator."
    );
    let changes_and_fixes = Release {
        version: "0.2.1".to_owned(),
        summary: "Hints come sooner and two problems are gone.".to_owned(),
        features: Vec::new(),
        changes: vec![Change {
            title: "Hints".to_owned(),
            audience: None,
            before: "Hints appeared 20 seconds into a round.".to_owned(),
            now: "Hints appear 15 seconds into a round.".to_owned(),
        }],
        fixes: vec![SOUND.to_owned(), "Fixed a missing hint.".to_owned()],
    };
    assert_eq!(
        changes_and_fixes.render(),
        "# Barnacle Update 0.2.1\n\
         Hints come sooner and two problems are gone.\n\
         \n\
         ## Changes to existing features\n\
         \n\
         ### Hints\n\
         **Before:** Hints appeared 20 seconds into a round.\n\
         **Now:** Hints appear 15 seconds into a round.\n\
         \n\
         ## Fixes\n\
         - Fixed a slow reply.\n\
         - Fixed a missing hint.\n\
         \n\
         -# Questions or feedback? Reach out to a server administrator."
    );
    Ok(())
}

fn titled(version: &str, title: Option<&str>) -> String {
    let title = title
        .map(|title| format!("title = \"{title}\"\n"))
        .unwrap_or_default();
    feature(
        version,
        &format!("{title}story = [\"{STORY}\"]\nsteps = [\"Press Skip.\"]\n"),
    )
}

fn audienced(version: &str, audience: &str) -> String {
    feature(
        version,
        &format!(
            "title = \"Skip a round\"\naudience = \"{audience}\"\nstory = [\"{STORY}\"]\nsteps = [\"Press Skip.\"]\n"
        ),
    )
}

fn title_fault(version: &str, title: &str, fault: TitleFault) -> NoteProblem {
    NoteProblem::Title {
        version: version.to_owned(),
        title: title.to_owned(),
        fault,
    }
}

fn audience_fault(version: &str, audience: &str, fault: TitleFault) -> NoteProblem {
    NoteProblem::Audience {
        version: version.to_owned(),
        title: "Skip a round".to_owned(),
        audience: audience.to_owned(),
        fault,
    }
}

fn post_length(text: &str) -> Result<usize, NotesError> {
    Ok(parse(text)?[0].render().chars().count() + 25)
}

#[test]
fn a_missing_or_malformed_title_or_an_oversized_post_is_reported() -> Result<(), NotesError> {
    let longest = format!("{}.", "a".repeat(119));
    let widest = "a".repeat(40);
    let too_wide = "a".repeat(41);
    let missing = NoteProblem::MissingTitle {
        version: "0.15.0".to_owned(),
        item: STORY.to_owned(),
    };
    let crowded = fixes("0.19.0", &vec![longest.as_str(); 16]);
    let crowded_length = post_length(&crowded)?;
    assert!(crowded_length > 2000, "{crowded_length}");
    let cases = [
        ("0.15.0", titled("0.15.0", None), vec![missing.clone()]),
        ("0.15.0", titled("0.15.0", Some("")), vec![missing]),
        (
            "0.16.0",
            titled("0.16.0", Some(&too_wide)),
            vec![title_fault(
                "0.16.0",
                &too_wide,
                TitleFault::TooLong { length: 41 },
            )],
        ),
        (
            "0.17.0",
            titled("0.17.0", Some("Skip a round.")),
            vec![title_fault(
                "0.17.0",
                "Skip a round.",
                TitleFault::FinalPunctuation,
            )],
        ),
        (
            "0.18.0",
            titled("0.18.0", Some("Skip::round")),
            vec![title_fault(
                "0.18.0",
                "Skip::round",
                TitleFault::Technical { sequence: "::" },
            )],
        ),
        (
            "0.18.1",
            audienced("0.18.1", ""),
            vec![audience_fault("0.18.1", "", TitleFault::Empty)],
        ),
        (
            "0.18.2",
            audienced("0.18.2", &too_wide),
            vec![audience_fault(
                "0.18.2",
                &too_wide,
                TitleFault::TooLong { length: 41 },
            )],
        ),
        (
            "0.18.3",
            audienced("0.18.3", "server managers."),
            vec![audience_fault(
                "0.18.3",
                "server managers.",
                TitleFault::FinalPunctuation,
            )],
        ),
        (
            "0.19.0",
            crowded,
            vec![NoteProblem::PostTooLong {
                version: "0.19.0".to_owned(),
                length: crowded_length,
            }],
        ),
    ];
    for (version, text, expected) in cases {
        let found = reported(&text)?;
        assert_eq!(found, expected, "{version}");
        names_its_version_and_line(&found, version);
    }
    assert_eq!(reported(&titled("0.20.0", Some(&widest)))?, Vec::new());
    assert_eq!(reported(&audienced("0.20.1", &widest))?, Vec::new());
    let full = vec![longest.as_str(); 14];
    let room = 2000 - post_length(&fixes("0.21.0", &full))? - "\n- ".len();
    let last = format!("{}.", "a".repeat(room - 1));
    let filled = fixes(
        "0.21.0",
        &full
            .iter()
            .copied()
            .chain(std::iter::once(last.as_str()))
            .collect::<Vec<&str>>(),
    );
    assert_eq!(post_length(&filled)?, 2000);
    assert_eq!(reported(&filled)?, Vec::new());
    Ok(())
}

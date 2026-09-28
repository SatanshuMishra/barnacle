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

fn fixed(version: &str, lines: &[&str]) -> String {
    let quoted: Vec<String> = lines.iter().map(|line| format!("'{line}'")).collect();
    format!(
        "[[release]]\nversion = \"{version}\"\nfixed = [{}]\n",
        quoted.join(", ")
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
            format!("{}{}", fixed("0.3.0", &[SOUND]), fixed("0.3.0", &[SOUND])),
            vec![NoteProblem::DuplicateVersion {
                version: "0.3.0".to_owned(),
            }],
        ),
        (
            "0.2",
            fixed("0.2", &[SOUND]),
            vec![NoteProblem::BadVersion {
                version: "0.2".to_owned(),
            }],
        ),
        (
            "0.4.0",
            "[[release]]\nversion = \"0.4.0\"\n".to_owned(),
            vec![NoteProblem::NoLines {
                version: "0.4.0".to_owned(),
            }],
        ),
        (
            "0.5.0",
            "[[release]]\nversion = \"0.5.0\"\n\n[[release.new]]\ntitle = \"Skip a round\"\nwhat = \"You can now skip a round.\"\nhow = \"\"\n".to_owned(),
            vec![NoteProblem::MissingHow {
                version: "0.5.0".to_owned(),
                what: "You can now skip a round.".to_owned(),
            }],
        ),
        (
            "0.6.0",
            "[[release]]\nversion = \"0.6.0\"\n\n[[release.new]]\ntitle = \"End a series\"\nwhat = \"You can now end a series.\"\n".to_owned(),
            vec![NoteProblem::MissingHow {
                version: "0.6.0".to_owned(),
                what: "You can now end a series.".to_owned(),
            }],
        ),
        (
            "0.7.0",
            fixed("0.7.0", &[&too_long]),
            vec![line("0.7.0", &too_long, LineFault::TooLong { length: 121 })],
        ),
        (
            "0.8.0",
            fixed("0.8.0", &["Fixed a slow reply"]),
            vec![line(
                "0.8.0",
                "Fixed a slow reply",
                LineFault::NoFinalPunctuation,
            )],
        ),
        (
            "0.9.0",
            fixed("0.9.0", &["Fixed it. Also more."]),
            vec![line(
                "0.9.0",
                "Fixed it. Also more.",
                LineFault::SentenceBreak,
            )],
        ),
        (
            "0.10.0",
            fixed("0.10.0", &["Fixed the `Table::start` race."]),
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
            fixed("0.11.0", &[""]),
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
            reported(&fixed("0.12.0", &[&text]))?,
            vec![line("0.12.0", &text, LineFault::Technical { sequence })]
        );
    }
    assert_eq!(reported(&fixed("0.13.0", &[&longest]))?, Vec::new());
    let crowded = vec![longest.as_str(); 60];
    let found = reported(&fixed("0.14.0", &crowded))?;
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
         ## New\n\
         \n\
         ### Guess series\n\
         You can now play a series of 2 to 20 silhouette rounds in a row without starting each one yourself.\n\
         > **How to use:** Type /guess-series, choose how many rounds you want (10 if you leave it out), and answer in chat as usual.\n\
         \n\
         ### Series standings\n\
         After every round in a series, Barnacle shows who has won the most rounds and who was fastest so far.\n\
         > **How to use:** Keep playing; the standings appear under each answer, with a 5-second countdown before the next ship.\n\
         \n\
         ### Still playing check\n\
         If the chat goes quiet for three rounds, Barnacle asks if you are still playing and stops unless someone presses Yes.\n\
         > **How to use:** Press Yes within 10 seconds to keep the series going.\n\
         \n\
         ### Update news\n\
         Server managers can now choose a channel where Barnacle posts news about its updates.\n\
         > **How to use:** Someone with Manage Server types /updates setup, picks the channel, and can add a role to notify.\n\
         \n\
         -# Questions or feedback? Reach out to a server administrator."
    );
    assert_eq!(
        FOOTER,
        "Questions or feedback? Reach out to a server administrator."
    );
    let fixes_only = Release {
        version: "0.2.1".to_owned(),
        new: Vec::new(),
        changed: vec!["Hints now come sooner.".to_owned()],
        fixed: vec![SOUND.to_owned(), "Fixed a missing hint.".to_owned()],
    };
    assert_eq!(
        fixes_only.render(),
        "# Barnacle Update 0.2.1\n\
         \n\
         ## Changed\n\
         - Hints now come sooner.\n\
         \n\
         ## Fixed\n\
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
    format!(
        "[[release]]\nversion = \"{version}\"\n\n[[release.new]]\n{title}what = \"You can now skip a round.\"\nhow = \"Press Skip.\"\n"
    )
}

fn title_fault(version: &str, title: &str, fault: TitleFault) -> NoteProblem {
    NoteProblem::Title {
        version: version.to_owned(),
        what: "You can now skip a round.".to_owned(),
        title: title.to_owned(),
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
        what: "You can now skip a round.".to_owned(),
    };
    let crowded = fixed("0.19.0", &vec![longest.as_str(); 16]);
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
    let full = vec![longest.as_str(); 15];
    let room = 2000 - post_length(&fixed("0.21.0", &full))? - "\n- ".len();
    let last = format!("{}.", "a".repeat(room - 1));
    let filled = fixed(
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

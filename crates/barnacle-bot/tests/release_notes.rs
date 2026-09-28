use barnacle_bot::release_notes::FOOTER;
use barnacle_bot::release_notes::LineFault;
use barnacle_bot::release_notes::NoteProblem;
use barnacle_bot::release_notes::NotesError;
use barnacle_bot::release_notes::Release;
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
            "[[release]]\nversion = \"0.5.0\"\n\n[[release.new]]\nwhat = \"You can now skip a round.\"\nhow = \"\"\n".to_owned(),
            vec![NoteProblem::MissingHow {
                version: "0.5.0".to_owned(),
                what: "You can now skip a round.".to_owned(),
            }],
        ),
        (
            "0.6.0",
            "[[release]]\nversion = \"0.6.0\"\n\n[[release.new]]\nwhat = \"You can now end a series.\"\n".to_owned(),
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
    assert!(found.contains(&NoteProblem::FieldTooLong {
        version: "0.14.0".to_owned(),
        field: "Fixed",
        length: 7379,
    }));
    assert!(
        found
            .iter()
            .any(|problem| matches!(problem, NoteProblem::EmbedTooLong { version, .. } if version == "0.14.0"))
    );
    Ok(())
}

#[test]
fn a_release_renders_its_heading_sections_and_footer() -> Result<(), NotesError> {
    let releases = all()?;
    let release = find(&releases, "0.2.0").unwrap();
    assert_eq!(release.title(), "Barnacle Update 0.2.0");
    let fields = release.fields();
    assert_eq!(fields.len(), 1);
    let (name, text) = &fields[0];
    assert_eq!(*name, "New");
    assert!(text.starts_with("- You can now play a series"));
    assert!(text.contains("How to use: Type /guess-series"));
    let rows: Vec<&str> = text.lines().collect();
    assert_eq!(rows.len(), 8);
    assert!(rows.iter().step_by(2).all(|row| row.starts_with("- ")));
    assert!(
        rows.iter()
            .skip(1)
            .step_by(2)
            .all(|row| row.starts_with("How to use: "))
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
        fixes_only.fields(),
        vec![
            ("Changed", "- Hints now come sooner.".to_owned()),
            (
                "Fixed",
                "- Fixed a slow reply.\n- Fixed a missing hint.".to_owned()
            ),
        ]
    );
    Ok(())
}

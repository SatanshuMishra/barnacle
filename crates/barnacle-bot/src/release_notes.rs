use std::fmt;

use serde::Deserialize;

use crate::text::MESSAGE_CONTENT_LIMIT;

pub const FOOTER: &str = "Questions or feedback? Reach out to a server administrator.";

const NOTES: &str = include_str!("../release-notes.toml");
const LONGEST_LINE: usize = 120;
const LONGEST_TITLE: usize = 40;
const LONGEST_PING_LINE: usize = 25;
const SENTENCE_ENDS: [char; 3] = ['.', '!', '?'];
const TECHNICAL: [&str; 11] = [
    "`", "**", "__", "::", "->", "http", ".rs", ".toml", ".sql", "{", "}",
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub version: String,
    #[serde(default)]
    pub new: Vec<NewItem>,
    #[serde(default)]
    pub changed: Vec<String>,
    #[serde(default)]
    pub fixed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewItem {
    #[serde(default)]
    pub title: String,
    pub what: String,
    #[serde(default)]
    pub how: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NotesFile {
    #[serde(default)]
    release: Vec<Release>,
}

#[derive(Debug, thiserror::Error)]
pub enum NotesError {
    #[error("the release notes are not valid TOML")]
    Toml(#[from] toml::de::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineFault {
    Empty,
    TooLong { length: usize },
    NoFinalPunctuation,
    SentenceBreak,
    Technical { sequence: &'static str },
}

impl fmt::Display for LineFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("is empty"),
            Self::TooLong { length } => {
                write!(f, "is {length} characters; the limit is {LONGEST_LINE}")
            }
            Self::NoFinalPunctuation => f.write_str("does not end in '.', '!' or '?'"),
            Self::SentenceBreak => f.write_str("holds more than one sentence"),
            Self::Technical { sequence } => write!(f, "contains {sequence:?}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleFault {
    TooLong { length: usize },
    FinalPunctuation,
    Technical { sequence: &'static str },
}

impl fmt::Display for TitleFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { length } => {
                write!(f, "is {length} characters; the limit is {LONGEST_TITLE}")
            }
            Self::FinalPunctuation => f.write_str("ends in '.', '!' or '?'"),
            Self::Technical { sequence } => write!(f, "contains {sequence:?}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NoteProblem {
    #[error("{version} has more than one entry")]
    DuplicateVersion { version: String },
    #[error(
        "\"{version}\" is not a version; write three whole numbers separated by dots, such as 0.2.0"
    )]
    BadVersion { version: String },
    #[error("{version} has no new, changed or fixed line")]
    NoLines { version: String },
    #[error("{version}: the new item \"{what}\" has no how")]
    MissingHow { version: String, what: String },
    #[error("{version}: the new item \"{what}\" has no title")]
    MissingTitle { version: String, what: String },
    #[error("{version}: the title \"{title}\" of the new item \"{what}\" {fault}")]
    Title {
        version: String,
        what: String,
        title: String,
        fault: TitleFault,
    },
    #[error("{version}: \"{line}\" {fault}")]
    Line {
        version: String,
        line: String,
        fault: LineFault,
    },
    #[error(
        "{version}: the post with the longest ping is {length} characters; Discord allows {MESSAGE_CONTENT_LIMIT}"
    )]
    PostTooLong { version: String, length: usize },
}

impl Release {
    pub fn render(&self) -> String {
        let new = if self.new.is_empty() {
            Vec::new()
        } else {
            std::iter::once("## New".to_owned())
                .chain(self.new.iter().flat_map(|item| {
                    [
                        String::new(),
                        format!("### {}", item.title),
                        item.what.clone(),
                        format!("> **How to use:** {}", item.how),
                    ]
                }))
                .collect()
        };
        std::iter::once(format!("# Barnacle Update {}", self.version))
            .chain(new)
            .chain(section("Changed", &self.changed))
            .chain(section("Fixed", &self.fixed))
            .chain([String::new(), format!("-# {FOOTER}")])
            .collect::<Vec<String>>()
            .join("\n")
    }

    fn lines(&self) -> impl Iterator<Item = &str> {
        self.new
            .iter()
            .flat_map(|item| {
                std::iter::once(item.what.as_str())
                    .chain(Some(item.how.as_str()).filter(|how| !how.trim().is_empty()))
            })
            .chain(self.changed.iter().map(String::as_str))
            .chain(self.fixed.iter().map(String::as_str))
    }
}

pub fn parse(text: &str) -> Result<Vec<Release>, NotesError> {
    let file: NotesFile = toml::from_str(text)?;
    Ok(file.release)
}

pub fn all() -> Result<Vec<Release>, NotesError> {
    parse(NOTES)
}

pub fn find<'a>(releases: &'a [Release], version: &str) -> Option<&'a Release> {
    releases.iter().find(|release| release.version == version)
}

pub fn problems(releases: &[Release]) -> Vec<NoteProblem> {
    releases
        .iter()
        .enumerate()
        .flat_map(|(position, release)| {
            releases[..position]
                .iter()
                .any(|earlier| earlier.version == release.version)
                .then(|| NoteProblem::DuplicateVersion {
                    version: release.version.clone(),
                })
                .into_iter()
                .chain(release_problems(release))
        })
        .collect()
}

fn release_problems(release: &Release) -> Vec<NoteProblem> {
    let version = || release.version.clone();
    let bad_version =
        (!is_version(&release.version)).then(|| NoteProblem::BadVersion { version: version() });
    let no_lines =
        (release.new.is_empty() && release.changed.is_empty() && release.fixed.is_empty())
            .then(|| NoteProblem::NoLines { version: version() });
    let missing_how = release
        .new
        .iter()
        .filter(|item| item.how.trim().is_empty())
        .map(|item| NoteProblem::MissingHow {
            version: version(),
            what: item.what.clone(),
        });
    let titles = release.new.iter().flat_map(|item| {
        let missing = item
            .title
            .trim()
            .is_empty()
            .then(|| NoteProblem::MissingTitle {
                version: version(),
                what: item.what.clone(),
            });
        missing
            .into_iter()
            .chain(
                title_faults(&item.title)
                    .into_iter()
                    .map(|fault| NoteProblem::Title {
                        version: version(),
                        what: item.what.clone(),
                        title: item.title.clone(),
                        fault,
                    }),
            )
    });
    let lines = release.lines().flat_map(|line| {
        faults(line).into_iter().map(|fault| NoteProblem::Line {
            version: version(),
            line: line.to_owned(),
            fault,
        })
    });
    let length = release.render().chars().count() + LONGEST_PING_LINE;
    let too_long = (length > MESSAGE_CONTENT_LIMIT).then(|| NoteProblem::PostTooLong {
        version: version(),
        length,
    });
    bad_version
        .into_iter()
        .chain(no_lines)
        .chain(missing_how)
        .chain(titles)
        .chain(lines)
        .chain(too_long)
        .collect()
}

fn title_faults(title: &str) -> Vec<TitleFault> {
    let length = title.chars().count();
    let too_long = (length > LONGEST_TITLE).then_some(TitleFault::TooLong { length });
    let punctuated = title
        .ends_with(SENTENCE_ENDS)
        .then_some(TitleFault::FinalPunctuation);
    too_long
        .into_iter()
        .chain(punctuated)
        .chain(technical(title).map(|sequence| TitleFault::Technical { sequence }))
        .collect()
}

fn faults(line: &str) -> Vec<LineFault> {
    if line.trim().is_empty() {
        return vec![LineFault::Empty];
    }
    let length = line.chars().count();
    let too_long = (length > LONGEST_LINE).then_some(LineFault::TooLong { length });
    let unfinished = (!line.ends_with(SENTENCE_ENDS)).then_some(LineFault::NoFinalPunctuation);
    let broken = has_sentence_break(line).then_some(LineFault::SentenceBreak);
    too_long
        .into_iter()
        .chain(unfinished)
        .chain(broken)
        .chain(technical(line).map(|sequence| LineFault::Technical { sequence }))
        .collect()
}

fn technical(text: &str) -> impl Iterator<Item = &'static str> {
    TECHNICAL
        .into_iter()
        .filter(move |sequence| text.contains(sequence))
}

fn has_sentence_break(line: &str) -> bool {
    line.chars()
        .zip(line.chars().skip(1))
        .any(|(end, next)| SENTENCE_ENDS.contains(&end) && next.is_whitespace())
}

fn is_version(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| is_number(part))
}

fn is_number(part: &str) -> bool {
    !part.is_empty()
        && part.bytes().all(|byte| byte.is_ascii_digit())
        && (part == "0" || !part.starts_with('0'))
}

fn section(heading: &str, lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        Vec::new()
    } else {
        [String::new(), format!("## {heading}")]
            .into_iter()
            .chain(bullets(lines))
            .collect()
    }
}

fn bullets(lines: &[String]) -> Vec<String> {
    lines.iter().map(|line| format!("- {line}")).collect()
}

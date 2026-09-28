use std::fmt;

use serde::Deserialize;

pub const FOOTER: &str = "Questions or feedback? Reach out to a server administrator.";

const NOTES: &str = include_str!("../release-notes.toml");
const LONGEST_LINE: usize = 120;
const FIELD_LIMIT: usize = 1024;
const EMBED_LIMIT: usize = 6000;
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
    #[error("{version}: \"{line}\" {fault}")]
    Line {
        version: String,
        line: String,
        fault: LineFault,
    },
    #[error("{version}: the {field} field is {length} characters; Discord allows {FIELD_LIMIT}")]
    FieldTooLong {
        version: String,
        field: &'static str,
        length: usize,
    },
    #[error("{version}: the announcement is {length} characters; Discord allows {EMBED_LIMIT}")]
    EmbedTooLong { version: String, length: usize },
}

impl Release {
    pub fn title(&self) -> String {
        format!("Barnacle Update {}", self.version)
    }

    pub fn fields(&self) -> Vec<(&'static str, String)> {
        let new: Vec<String> = self
            .new
            .iter()
            .map(|item| format!("- {}\nHow to use: {}", item.what, item.how))
            .collect();
        [
            ("New", new),
            ("Changed", bullets(&self.changed)),
            ("Fixed", bullets(&self.fixed)),
        ]
        .into_iter()
        .filter(|(_, lines)| !lines.is_empty())
        .map(|(name, lines)| (name, lines.join("\n")))
        .collect()
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
    let lines = release.lines().flat_map(|line| {
        faults(line).into_iter().map(|fault| NoteProblem::Line {
            version: version(),
            line: line.to_owned(),
            fault,
        })
    });
    bad_version
        .into_iter()
        .chain(no_lines)
        .chain(missing_how)
        .chain(lines)
        .chain(embed_problems(release))
        .collect()
}

fn embed_problems(release: &Release) -> Vec<NoteProblem> {
    let fields = release.fields();
    let long_fields = fields.iter().filter_map(|(field, value)| {
        let length = value.chars().count();
        (length > FIELD_LIMIT).then(|| NoteProblem::FieldTooLong {
            version: release.version.clone(),
            field,
            length,
        })
    });
    let length = release.title().chars().count()
        + fields
            .iter()
            .map(|(field, value)| field.chars().count() + value.chars().count())
            .sum::<usize>()
        + FOOTER.chars().count();
    let long_embed = (length > EMBED_LIMIT).then(|| NoteProblem::EmbedTooLong {
        version: release.version.clone(),
        length,
    });
    long_fields.chain(long_embed).collect()
}

fn faults(line: &str) -> Vec<LineFault> {
    if line.trim().is_empty() {
        return vec![LineFault::Empty];
    }
    let length = line.chars().count();
    let too_long = (length > LONGEST_LINE).then_some(LineFault::TooLong { length });
    let unfinished = (!line.ends_with(SENTENCE_ENDS)).then_some(LineFault::NoFinalPunctuation);
    let broken = has_sentence_break(line).then_some(LineFault::SentenceBreak);
    let technical = TECHNICAL
        .into_iter()
        .filter(|sequence| line.contains(sequence))
        .map(|sequence| LineFault::Technical { sequence });
    too_long
        .into_iter()
        .chain(unfinished)
        .chain(broken)
        .chain(technical)
        .collect()
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

fn bullets(lines: &[String]) -> Vec<String> {
    lines.iter().map(|line| format!("- {line}")).collect()
}

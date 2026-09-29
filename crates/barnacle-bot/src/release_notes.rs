use std::fmt;

use serde::Deserialize;

use crate::text::MESSAGE_CONTENT_LIMIT;

pub const FOOTER: &str = "Questions or feedback? Reach out to a server administrator.";

const NOTES: &str = include_str!("../release-notes.toml");
const LONGEST_LINE: usize = 120;
const LONGEST_TITLE: usize = 40;
const LONGEST_STORY: usize = 2;
const LONGEST_PING_LINE: usize = 25;
const SENTENCE_ENDS: [char; 3] = ['.', '!', '?'];
const TECHNICAL: [&str; 11] = [
    "`", "**", "__", "::", "->", "http", ".rs", ".toml", ".sql", "{", "}",
];
const NEW_FEATURES: &str = "## New features";
const CHANGES: &str = "## Changes to existing features";
const FIXES: &str = "## Fixes";
const HOW_TO_USE: &str = "**How to use it**";
const WHAT_TO_EXPECT: &str = "**What to expect**";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub version: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default, rename = "feature")]
    pub features: Vec<Feature>,
    #[serde(default, rename = "change")]
    pub changes: Vec<Change>,
    #[serde(default)]
    pub fixes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feature {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub audience: Option<String>,
    #[serde(default)]
    pub story: Vec<String>,
    #[serde(default)]
    pub steps: Vec<String>,
    #[serde(default)]
    pub expect: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub audience: Option<String>,
    #[serde(default)]
    pub before: String,
    #[serde(default)]
    pub now: String,
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
    Empty,
    TooLong { length: usize },
    FinalPunctuation,
    Technical { sequence: &'static str },
}

impl fmt::Display for TitleFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("is empty"),
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
    #[error("{version} has no summary; say in one sentence what the update brings")]
    MissingSummary { version: String },
    #[error("{version} has no feature, change or fix")]
    NoContent { version: String },
    #[error("{version}: the item \"{item}\" has no title")]
    MissingTitle { version: String, item: String },
    #[error("{version}: the title \"{title}\" {fault}")]
    Title {
        version: String,
        title: String,
        fault: TitleFault,
    },
    #[error("{version}: the audience \"{audience}\" of \"{title}\" {fault}")]
    Audience {
        version: String,
        title: String,
        audience: String,
        fault: TitleFault,
    },
    #[error("{version}: the feature \"{title}\" has no story; say what it is and why it helps")]
    MissingStory { version: String, title: String },
    #[error(
        "{version}: the feature \"{title}\" tells its story in {sentences} sentences; use at most {LONGEST_STORY}"
    )]
    LongStory {
        version: String,
        title: String,
        sentences: usize,
    },
    #[error("{version}: the feature \"{title}\" has no steps; say how to use it")]
    MissingSteps { version: String, title: String },
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
        std::iter::once(format!("# Barnacle Update {}", self.version))
            .chain(Some(self.summary.clone()).filter(|summary| !summary.trim().is_empty()))
            .chain(section(
                NEW_FEATURES,
                self.features.iter().map(Feature::block),
            ))
            .chain(section(CHANGES, self.changes.iter().map(Change::block)))
            .chain(fixes(&self.fixes))
            .chain([String::new(), format!("-# {FOOTER}")])
            .collect::<Vec<String>>()
            .join("\n")
    }

    fn lines(&self) -> impl Iterator<Item = &str> {
        Some(self.summary.as_str())
            .filter(|summary| !summary.trim().is_empty())
            .into_iter()
            .chain(self.features.iter().flat_map(|feature| {
                feature
                    .story
                    .iter()
                    .chain(&feature.steps)
                    .chain(&feature.expect)
                    .map(String::as_str)
            }))
            .chain(
                self.changes
                    .iter()
                    .flat_map(|change| [change.before.as_str(), change.now.as_str()]),
            )
            .chain(self.fixes.iter().map(String::as_str))
    }
}

impl Feature {
    fn block(&self) -> Vec<String> {
        let steps = labelled(
            HOW_TO_USE,
            self.steps
                .iter()
                .zip(1..)
                .map(|(step, number)| format!("{number}. {step}")),
        );
        let expect = labelled(
            WHAT_TO_EXPECT,
            self.expect.iter().map(|line| format!("- {line}")),
        );
        heading(&self.title, self.audience.as_deref())
            .into_iter()
            .chain(Some(self.story.join(" ")).filter(|story| !story.is_empty()))
            .chain(steps)
            .chain(expect)
            .collect()
    }

    fn first_line(&self) -> &str {
        self.story
            .iter()
            .chain(&self.steps)
            .next()
            .map_or("", String::as_str)
    }
}

impl Change {
    fn block(&self) -> Vec<String> {
        heading(&self.title, self.audience.as_deref())
            .into_iter()
            .chain([
                format!("**Before:** {}", self.before),
                format!("**Now:** {}", self.now),
            ])
            .collect()
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
    let missing_summary = release
        .summary
        .trim()
        .is_empty()
        .then(|| NoteProblem::MissingSummary { version: version() });
    let no_content =
        (release.features.is_empty() && release.changes.is_empty() && release.fixes.is_empty())
            .then(|| NoteProblem::NoContent { version: version() });
    let headings = release
        .features
        .iter()
        .map(|feature| {
            (
                feature.title.as_str(),
                feature.audience.as_deref(),
                feature.first_line(),
            )
        })
        .chain(release.changes.iter().map(|change| {
            (
                change.title.as_str(),
                change.audience.as_deref(),
                change.now.as_str(),
            )
        }))
        .flat_map(|(title, audience, item)| {
            heading_problems(&release.version, title, audience, item)
        });
    let gaps = release
        .features
        .iter()
        .flat_map(|feature| feature_problems(&release.version, feature));
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
        .chain(missing_summary)
        .chain(no_content)
        .chain(headings)
        .chain(gaps)
        .chain(lines)
        .chain(too_long)
        .collect()
}

fn heading_problems(
    version: &str,
    title: &str,
    audience: Option<&str>,
    item: &str,
) -> Vec<NoteProblem> {
    let titled = if title.trim().is_empty() {
        vec![NoteProblem::MissingTitle {
            version: version.to_owned(),
            item: item.to_owned(),
        }]
    } else {
        title_faults(title)
            .into_iter()
            .map(|fault| NoteProblem::Title {
                version: version.to_owned(),
                title: title.to_owned(),
                fault,
            })
            .collect()
    };
    let audienced = audience.into_iter().flat_map(|audience| {
        title_faults(audience)
            .into_iter()
            .map(move |fault| NoteProblem::Audience {
                version: version.to_owned(),
                title: title.to_owned(),
                audience: audience.to_owned(),
                fault,
            })
    });
    titled.into_iter().chain(audienced).collect()
}

fn feature_problems(version: &str, feature: &Feature) -> Vec<NoteProblem> {
    let title = || feature.title.clone();
    let story = match feature.story.len() {
        0 => Some(NoteProblem::MissingStory {
            version: version.to_owned(),
            title: title(),
        }),
        sentences if sentences > LONGEST_STORY => Some(NoteProblem::LongStory {
            version: version.to_owned(),
            title: title(),
            sentences,
        }),
        _ => None,
    };
    let steps = feature.steps.is_empty().then(|| NoteProblem::MissingSteps {
        version: version.to_owned(),
        title: title(),
    });
    story.into_iter().chain(steps).collect()
}

fn title_faults(title: &str) -> Vec<TitleFault> {
    if title.trim().is_empty() {
        return vec![TitleFault::Empty];
    }
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

fn heading(title: &str, audience: Option<&str>) -> Vec<String> {
    std::iter::once(format!("### {title}"))
        .chain(audience.map(|audience| format!("-# For {audience}")))
        .collect()
}

fn labelled(label: &str, lines: impl Iterator<Item = String>) -> Vec<String> {
    let lines: Vec<String> = lines.collect();
    if lines.is_empty() {
        lines
    } else {
        std::iter::once(label.to_owned()).chain(lines).collect()
    }
}

fn section(heading: &str, blocks: impl Iterator<Item = Vec<String>>) -> Vec<String> {
    let blocks: Vec<Vec<String>> = blocks.collect();
    if blocks.is_empty() {
        Vec::new()
    } else {
        [String::new(), heading.to_owned()]
            .into_iter()
            .chain(
                blocks
                    .into_iter()
                    .flat_map(|block| std::iter::once(String::new()).chain(block)),
            )
            .collect()
    }
}

fn fixes(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        Vec::new()
    } else {
        [String::new(), FIXES.to_owned()]
            .into_iter()
            .chain(lines.iter().map(|line| format!("- {line}")))
            .collect()
    }
}

use barnacle_guess::UserId;
use jiff::civil::Date;

use crate::schedule::Zone;
use crate::wiring::same_name;

pub const VALID_MARK: &str = "\u{2705}";
pub const INVALID_MARK: &str = "\u{274C}";

const SCREENSHOTS: usize = 2;
const TYPED_LIMIT: usize = 100;
const ELLIPSIS: char = '\u{2026}';
const BOARD_LINES: usize = 10;
const ADJUSTMENT_LIMIT: i64 = 1000;
const MESSAGE_LIMIT: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Criterion {
    pub name: String,
    pub description: String,
    pub points: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub content_type: Option<String>,
}

impl Attachment {
    fn is_image(&self) -> bool {
        self.content_type
            .as_deref()
            .is_some_and(|content_type| content_type.starts_with("image/"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    NoName { example: String },
    UnknownName { typed: String, closest: String },
    ScreenshotCount(usize),
    NotAnImage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checked {
    Valid { criterion: String, points: u32 },
    Invalid(Vec<Problem>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Stopped,
    Completed,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JudgingReaction {
    #[default]
    Trophy,
    Star,
    Hundred,
    Crown,
}

impl JudgingReaction {
    pub const ALL: [Self; 4] = [Self::Trophy, Self::Star, Self::Hundred, Self::Crown];

    pub fn emoji(self) -> &'static str {
        match self {
            Self::Trophy => "\u{1F3C6}",
            Self::Star => "\u{2B50}",
            Self::Hundred => "\u{1F4AF}",
            Self::Crown => "\u{1F451}",
        }
    }

    pub fn value(self) -> &'static str {
        match self {
            Self::Trophy => "trophy",
            Self::Star => "star",
            Self::Hundred => "hundred",
            Self::Crown => "crown",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reaction| reaction.value() == text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    pub user: UserId,
    pub points: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    pub rank: usize,
    pub user: UserId,
    pub points: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardRow {
    pub rank: Option<usize>,
    pub user: UserId,
    pub points: i64,
    pub requester: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartChoice {
    Setup,
    Resume,
    AlreadyRunning,
    NeedsCleanup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompleteRefusal {
    TooEarly,
    NotLive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupRefusal {
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckPath {
    Live,
    CatchUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Done {
    JudgesChanged,
    ChannelsUpdated,
    AnnouncementUpdated,
    SettingsSaved,
    Completed,
    ChannelsDeleted,
    Archived,
}

pub fn check_submission(text: &str, attachments: &[Attachment], criteria: &[Criterion]) -> Checked {
    let typed = text.trim();
    let matched = criteria
        .iter()
        .find(|criterion| same_name(&criterion.name, typed));
    let name_problem = match matched {
        Some(_) => None,
        None if typed.is_empty() => criteria.first().map(|first| Problem::NoName {
            example: first.name.clone(),
        }),
        None => closest(typed, criteria).map(|closest| Problem::UnknownName {
            typed: cut(typed),
            closest: closest.name.clone(),
        }),
    };
    let images = attachments
        .iter()
        .filter(|attachment| attachment.is_image())
        .count();
    let count_problem = (images != SCREENSHOTS).then_some(Problem::ScreenshotCount(images));
    let type_problem = (images != attachments.len()).then_some(Problem::NotAnImage);
    let problems: Vec<Problem> = [name_problem, count_problem, type_problem]
        .into_iter()
        .flatten()
        .collect();
    match (matched, problems.is_empty()) {
        (Some(criterion), true) => Checked::Valid {
            criterion: criterion.name.clone(),
            points: criterion.points,
        },
        _ => Checked::Invalid(problems),
    }
}

fn cut(typed: &str) -> String {
    if typed.chars().count() > TYPED_LIMIT {
        typed.chars().take(TYPED_LIMIT).chain([ELLIPSIS]).collect()
    } else {
        typed.to_owned()
    }
}

fn closest<'a>(typed: &str, criteria: &'a [Criterion]) -> Option<&'a Criterion> {
    let typed = typed.to_lowercase();
    criteria
        .iter()
        .enumerate()
        .min_by_key(|(position, criterion)| {
            (
                edit_distance(&typed, &criterion.name.to_lowercase()),
                *position,
            )
        })
        .map(|(_, criterion)| criterion)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let first: Vec<usize> = (0..=b.len()).collect();
    let last = a.chars().enumerate().fold(first, |previous, (row, left)| {
        b.iter()
            .enumerate()
            .fold(vec![row + 1], |mut current, (column, right)| {
                let substitution = previous[column] + usize::from(left != *right);
                let deletion = previous[column + 1] + 1;
                let insertion = current[column] + 1;
                current.push(substitution.min(deletion).min(insertion));
                current
            })
    });
    last[b.len()]
}

pub fn standings(scores: &[Score]) -> Vec<Standing> {
    let mut ordered: Vec<Score> = scores
        .iter()
        .copied()
        .filter(|score| score.points != 0)
        .collect();
    ordered.sort_by(|a, b| b.points.cmp(&a.points).then(a.user.cmp(&b.user)));
    ordered
        .iter()
        .enumerate()
        .map(|(position, score)| {
            let first_with_points = ordered
                .iter()
                .position(|other| other.points == score.points)
                .unwrap_or(position);
            Standing {
                rank: first_with_points + 1,
                user: score.user,
                points: score.points,
            }
        })
        .collect()
}

pub fn board_rows(standings: &[Standing], requester: UserId) -> Vec<BoardRow> {
    let to_row = |standing: &Standing| BoardRow {
        rank: Some(standing.rank),
        user: standing.user,
        points: standing.points,
        requester: standing.user == requester,
    };
    let own = standings
        .iter()
        .position(|standing| standing.user == requester);
    match own {
        Some(position) if position < BOARD_LINES => {
            standings.iter().take(BOARD_LINES).map(to_row).collect()
        }
        Some(position) => standings
            .iter()
            .take(BOARD_LINES - 1)
            .chain([&standings[position]])
            .map(to_row)
            .collect(),
        None => standings
            .iter()
            .take(BOARD_LINES - 1)
            .map(to_row)
            .chain([BoardRow {
                rank: None,
                user: requester,
                points: 0,
                requester: true,
            }])
            .collect(),
    }
}

pub fn winners(standings: &[Standing]) -> Vec<Standing> {
    standings
        .iter()
        .copied()
        .filter(|standing| standing.rank == 1)
        .collect()
}

pub fn start_choice(live: Option<Status>) -> StartChoice {
    match live {
        None | Some(Status::Archived) => StartChoice::Setup,
        Some(Status::Stopped) => StartChoice::Resume,
        Some(Status::Running) => StartChoice::AlreadyRunning,
        Some(Status::Completed) => StartChoice::NeedsCleanup,
    }
}

pub fn end_unix(last_day: Date, zone: &Zone) -> i64 {
    zone.midnight_after(last_day)
}

pub fn may_complete(status: Status, end_unix: i64, now_unix: i64) -> Result<(), CompleteRefusal> {
    match status {
        Status::Running | Status::Stopped if now_unix >= end_unix => Ok(()),
        Status::Running | Status::Stopped => Err(CompleteRefusal::TooEarly),
        Status::Completed | Status::Archived => Err(CompleteRefusal::NotLive),
    }
}

pub fn may_clean_up(status: Status) -> Result<(), CleanupRefusal> {
    match status {
        Status::Running => Err(CleanupRefusal::Running),
        Status::Stopped | Status::Completed | Status::Archived => Ok(()),
    }
}

pub fn should_check(status: Status, end_unix: i64, sent_unix: i64) -> bool {
    match status {
        Status::Running => sent_unix < end_unix,
        Status::Stopped | Status::Completed | Status::Archived => false,
    }
}

pub fn moves_last_seen(path: CheckPath) -> bool {
    matches!(path, CheckPath::CatchUp)
}

pub fn adjustment(points: i64) -> Option<i64> {
    (points != 0 && (-ADJUSTMENT_LIMIT..=ADJUSTMENT_LIMIT).contains(&points)).then_some(points)
}

pub fn message_chunks(text: &str) -> Vec<String> {
    if text.chars().count() <= MESSAGE_LIMIT {
        return vec![text.to_owned()];
    }
    let pieces = text.split('\n').flat_map(line_pieces);
    let (mut chunks, last) = pieces.fold(
        (Vec::new(), None::<Piece>),
        |(mut chunks, current), piece| match current {
            None => (chunks, Some(piece)),
            Some(open) if open.joins(&piece) => (chunks, Some(open.join(piece))),
            Some(open) => {
                chunks.push(open.text);
                (chunks, Some(piece))
            }
        },
    );
    chunks.extend(last.map(|open| open.text));
    chunks
}

struct Piece {
    text: String,
    size: usize,
    line_start: bool,
}

impl Piece {
    fn joins(&self, next: &Piece) -> bool {
        next.line_start && self.size + 1 + next.size <= MESSAGE_LIMIT
    }

    fn join(self, next: Piece) -> Piece {
        Piece {
            text: format!("{}\n{}", self.text, next.text),
            size: self.size + 1 + next.size,
            line_start: self.line_start,
        }
    }
}

fn line_pieces(line: &str) -> Vec<Piece> {
    let characters: Vec<char> = line.chars().collect();
    if characters.is_empty() {
        return vec![Piece {
            text: String::new(),
            size: 0,
            line_start: true,
        }];
    }
    characters
        .chunks(MESSAGE_LIMIT)
        .enumerate()
        .map(|(index, part)| Piece {
            text: part.iter().collect(),
            size: part.len(),
            line_start: index == 0,
        })
        .collect()
}

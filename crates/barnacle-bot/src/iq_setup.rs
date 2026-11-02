use std::num::NonZeroU64;

use barnacle_guess::UserId;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use poise::serenity_prelude as serenity;

use crate::event_draft::FIRST_DAY_INPUT;
use crate::event_draft::LAST_DAY_INPUT;
use crate::event_draft::PanelView;
use crate::event_draft::read_day;
use crate::event_draft::within_window;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::RoleId;
use crate::iq::Criterion;
use crate::iq::JudgingMarks;
use crate::iq::Mark;
use crate::iq_store::Competition;
pub use crate::iq_store::ConfigChange;
use crate::iq_store::NewCompetition;
use crate::iq_text;
use crate::panels;
use crate::panels::Owned;
use crate::schedule::Schedule;
use crate::schedule::Span;
use crate::schedule::Zone;
use crate::wiring::IqPanelPart;
use crate::wiring::iq_panel_id;
use crate::wiring::same_name;

pub type Drafts = panels::Drafts<SetupDraft>;

pub const BLANK_TEMPLATE: &str = "blank";
pub const NAME_INPUT: &str = "name";
pub const DESCRIPTION_INPUT: &str = "description";
pub const POINTS_INPUT: &str = "points";
pub const RULES_INPUT: &str = "rules";
pub const CRITERIA_LIMIT: usize = 25;
pub const ROLE_LIMIT: usize = 25;
pub const JUDGE_LIMIT: usize = 25;
pub const TEMPLATE_LIMIT: usize = 10;
pub const NAME_LIMIT: usize = 60;
pub const CRITERION_NAME_LIMIT: usize = 50;
pub const DESCRIPTION_LIMIT: usize = 300;
pub const RULES_LIMIT: usize = 4000;
pub const POINTS_LIMIT: u32 = 1000;

const CONTENT_LIMIT: usize = 2000;
const DAY_INPUT_MAX: u16 = 32;
const POINTS_INPUT_MAX: usize = 4;
const MENU_MAX: u8 = 25;
const QUOTE_MARK: &str = "> ";
const AWARDED_CHOICES: [&str; 4] = ["\u{1F3C6}", "\u{2B50}", "\u{1F4AF}", "\u{1F451}"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupStep {
    Basics,
    Criteria,
    Rules,
    Who,
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupRefusal {
    CriterionNameTaken(String),
    CriterionPoints,
    TooManyCriteria,
    CriterionName,
    CriterionDescription,
    CriterionUnknown,
    NameFormat,
    RulesTooLong,
    TooManyRoles,
    TooManyJudges,
    TemplateUnknown,
    DateFormat,
    DayNotOffered,
    LastDayNotAfterFirst,
    DatesTooFar,
    ReactionUnknown,
    FixedAfterStart,
    StepIncomplete(SetupStep),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetupChecks {
    pub role_below_judge: bool,
    pub missing: Vec<&'static str>,
}

impl SetupChecks {
    fn pass(&self) -> bool {
        !self.role_below_judge && self.missing.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Start,
    Edit { competition: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateChoice {
    Blank,
    Competition(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompetitionChannels {
    pub category: ChannelId,
    pub announcements: ChannelId,
    pub submissions: ChannelId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupPlan {
    pub guild: GuildId,
    pub name: String,
    pub first_day: Date,
    pub last_day: Date,
    pub zone: String,
    pub criteria: Vec<Criterion>,
    pub rules: String,
    pub roles: Vec<RoleId>,
    pub judges: Vec<UserId>,
    pub marks: JudgingMarks,
    pub created_by: UserId,
}

impl SetupPlan {
    pub fn competition(&self, channels: CompetitionChannels, created_at_ms: i64) -> NewCompetition {
        NewCompetition {
            guild: self.guild,
            name: self.name.clone(),
            first_day: self.first_day,
            last_day: self.last_day,
            zone: self.zone.clone(),
            category: channels.category,
            announcements: channels.announcements,
            submissions: channels.submissions,
            criteria: self.criteria.clone(),
            rules: self.rules.clone(),
            roles: self.roles.clone(),
            judges: self.judges.clone(),
            marks: self.marks.clone(),
            judge_log: None,
            created_by: self.created_by,
            created_at_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupReady {
    Start(SetupPlan),
    Edit(ConfigChange),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupDraft {
    purpose: Purpose,
    guild: GuildId,
    owner: UserId,
    templates: Vec<Competition>,
    today: Date,
    step: SetupStep,
    name: String,
    renamed: bool,
    first_day: Option<Date>,
    last_day: Option<Date>,
    zone: String,
    template: Option<TemplateChoice>,
    criteria: Vec<Criterion>,
    picked: Option<usize>,
    rules: String,
    roles: Vec<RoleId>,
    judges: Vec<UserId>,
    marks: JudgingMarks,
}

impl Owned for SetupDraft {
    fn owner(&self) -> UserId {
        self.owner
    }
}

impl SetupDraft {
    pub fn new(
        guild: GuildId,
        owner: UserId,
        templates: Vec<Competition>,
        zone: Zone,
        today: Date,
    ) -> SetupDraft {
        SetupDraft {
            purpose: Purpose::Start,
            guild,
            owner,
            templates: templates.into_iter().take(TEMPLATE_LIMIT).collect(),
            today,
            step: SetupStep::Basics,
            name: iq_text::competition_name(today),
            renamed: false,
            first_day: None,
            last_day: None,
            zone: zone.name().to_owned(),
            template: None,
            criteria: Vec::new(),
            picked: None,
            rules: String::new(),
            roles: Vec::new(),
            judges: Vec::new(),
            marks: JudgingMarks::default(),
        }
    }

    pub fn edit(competition: &Competition, owner: UserId) -> SetupDraft {
        SetupDraft {
            purpose: Purpose::Edit {
                competition: competition.id,
            },
            guild: competition.guild,
            owner,
            templates: Vec::new(),
            today: competition.first_day,
            step: SetupStep::Criteria,
            name: competition.name.clone(),
            renamed: true,
            first_day: Some(competition.first_day),
            last_day: Some(competition.last_day),
            zone: competition.zone.clone(),
            template: None,
            criteria: competition.criteria.clone(),
            picked: None,
            rules: competition.rules.clone(),
            roles: competition.roles.clone(),
            judges: competition.judges.clone(),
            marks: competition.marks.clone(),
        }
    }

    pub fn purpose(&self) -> Purpose {
        self.purpose
    }

    pub fn guild(&self) -> GuildId {
        self.guild
    }

    pub fn owner(&self) -> UserId {
        self.owner
    }

    pub fn step(&self) -> SetupStep {
        self.step
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn first_day(&self) -> Option<Date> {
        self.first_day
    }

    pub fn last_day(&self) -> Option<Date> {
        self.last_day
    }

    pub fn zone(&self) -> &str {
        &self.zone
    }

    pub fn template(&self) -> Option<TemplateChoice> {
        self.template
    }

    pub fn criteria(&self) -> &[Criterion] {
        &self.criteria
    }

    pub fn picked(&self) -> Option<usize> {
        self.picked
    }

    pub fn rules(&self) -> &str {
        &self.rules
    }

    pub fn roles(&self) -> &[RoleId] {
        &self.roles
    }

    pub fn judges(&self) -> &[UserId] {
        &self.judges
    }

    pub fn marks(&self) -> &JudgingMarks {
        &self.marks
    }

    pub fn with_template(&self, value: &str) -> Result<SetupDraft, SetupRefusal> {
        self.starting()?;
        if value == BLANK_TEMPLATE {
            return Ok(SetupDraft {
                template: Some(TemplateChoice::Blank),
                criteria: Vec::new(),
                picked: None,
                rules: String::new(),
                roles: Vec::new(),
                judges: Vec::new(),
                marks: JudgingMarks::default(),
                ..self.clone()
            });
        }
        let source = value
            .parse::<i64>()
            .ok()
            .and_then(|id| self.templates.iter().find(|template| template.id == id))
            .ok_or(SetupRefusal::TemplateUnknown)?;
        Ok(SetupDraft {
            template: Some(TemplateChoice::Competition(source.id)),
            criteria: source.criteria.clone(),
            picked: None,
            rules: source.rules.clone(),
            roles: source.roles.clone(),
            judges: source.judges.clone(),
            marks: source.marks.clone(),
            ..self.clone()
        })
    }

    pub fn with_typed_dates(&self, first: &str, last: &str) -> Result<SetupDraft, SetupRefusal> {
        self.starting()?;
        let first_day = read_day(first).map_err(|_| SetupRefusal::DateFormat)?;
        let last_day = read_day(last).map_err(|_| SetupRefusal::DateFormat)?;
        if first_day < self.today {
            return Err(SetupRefusal::DayNotOffered);
        }
        if last_day <= first_day {
            return Err(SetupRefusal::LastDayNotAfterFirst);
        }
        let typed = Span::new(first_day, last_day, Schedule::clan_battles())
            .ok_or(SetupRefusal::LastDayNotAfterFirst)?;
        let today_unix = start_of(self.today).ok_or(SetupRefusal::DatesTooFar)?;
        if !within_window(&typed, today_unix) {
            return Err(SetupRefusal::DatesTooFar);
        }
        let name = match self.renamed {
            true => self.name.clone(),
            false => iq_text::competition_name(first_day),
        };
        Ok(SetupDraft {
            first_day: Some(first_day),
            last_day: Some(last_day),
            name,
            ..self.clone()
        })
    }

    pub fn renamed(&self, text: &str) -> Result<SetupDraft, SetupRefusal> {
        self.starting()?;
        let name = text.trim();
        if !(1..=NAME_LIMIT).contains(&name.chars().count()) {
            return Err(SetupRefusal::NameFormat);
        }
        Ok(SetupDraft {
            name: name.to_owned(),
            renamed: true,
            ..self.clone()
        })
    }

    pub fn with_criterion(
        &self,
        name: &str,
        description: &str,
        points_text: &str,
        editing: Option<usize>,
    ) -> Result<SetupDraft, SetupRefusal> {
        if editing.is_some_and(|index| index >= self.criteria.len()) {
            return Err(SetupRefusal::CriterionUnknown);
        }
        if editing.is_none() && self.criteria.len() >= CRITERIA_LIMIT {
            return Err(SetupRefusal::TooManyCriteria);
        }
        let name = name.trim();
        if !(1..=CRITERION_NAME_LIMIT).contains(&name.chars().count()) {
            return Err(SetupRefusal::CriterionName);
        }
        let taken = self
            .criteria
            .iter()
            .enumerate()
            .any(|(index, held)| Some(index) != editing && same_name(&held.name, name));
        if taken {
            return Err(SetupRefusal::CriterionNameTaken(name.to_owned()));
        }
        let description = description.trim();
        if description.chars().count() > DESCRIPTION_LIMIT {
            return Err(SetupRefusal::CriterionDescription);
        }
        let points = read_points(points_text).ok_or(SetupRefusal::CriterionPoints)?;
        let entry = Criterion {
            name: name.to_owned(),
            description: description.to_owned(),
            points,
        };
        let criteria = match editing {
            Some(position) => self
                .criteria
                .iter()
                .enumerate()
                .map(|(index, held)| match index == position {
                    true => entry.clone(),
                    false => held.clone(),
                })
                .collect(),
            None => self
                .criteria
                .iter()
                .cloned()
                .chain(std::iter::once(entry))
                .collect(),
        };
        Ok(SetupDraft {
            criteria,
            picked: None,
            ..self.clone()
        })
    }

    pub fn with_picked_criterion(&self, index: usize) -> Result<SetupDraft, SetupRefusal> {
        if index >= self.criteria.len() {
            return Err(SetupRefusal::CriterionUnknown);
        }
        Ok(SetupDraft {
            picked: Some(index),
            ..self.clone()
        })
    }

    pub fn unpicked(&self) -> SetupDraft {
        SetupDraft {
            picked: None,
            ..self.clone()
        }
    }

    pub fn remove_picked(&self) -> Result<SetupDraft, SetupRefusal> {
        let position = self
            .picked
            .filter(|index| *index < self.criteria.len())
            .ok_or(SetupRefusal::CriterionUnknown)?;
        Ok(SetupDraft {
            criteria: self
                .criteria
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != position)
                .map(|(_, held)| held.clone())
                .collect(),
            picked: None,
            ..self.clone()
        })
    }

    pub fn with_rules(&self, text: &str) -> Result<SetupDraft, SetupRefusal> {
        let rules = text.trim();
        if rules.chars().count() > RULES_LIMIT {
            return Err(SetupRefusal::RulesTooLong);
        }
        Ok(SetupDraft {
            rules: rules.to_owned(),
            ..self.clone()
        })
    }

    pub fn with_roles(&self, roles: Vec<RoleId>) -> Result<SetupDraft, SetupRefusal> {
        if roles.len() > ROLE_LIMIT {
            return Err(SetupRefusal::TooManyRoles);
        }
        Ok(SetupDraft {
            roles,
            ..self.clone()
        })
    }

    pub fn with_judges(&self, judges: Vec<UserId>) -> Result<SetupDraft, SetupRefusal> {
        if judges.len() > JUDGE_LIMIT {
            return Err(SetupRefusal::TooManyJudges);
        }
        Ok(SetupDraft {
            judges,
            ..self.clone()
        })
    }

    pub fn with_reaction(&self, value: &str) -> Result<SetupDraft, SetupRefusal> {
        let awarded = AWARDED_CHOICES
            .into_iter()
            .find(|choice| *choice == value)
            .and_then(Mark::parse)
            .ok_or(SetupRefusal::ReactionUnknown)?;
        let marks = JudgingMarks::new(
            self.marks.denied.clone(),
            awarded,
            self.marks.confirm.clone(),
        )
        .map_err(|_| SetupRefusal::ReactionUnknown)?;
        Ok(SetupDraft {
            marks,
            ..self.clone()
        })
    }

    pub fn next(&self) -> Result<SetupDraft, SetupRefusal> {
        if !self.step_met(self.step) {
            return Err(SetupRefusal::StepIncomplete(self.step));
        }
        let step = match self.step {
            SetupStep::Basics => SetupStep::Criteria,
            SetupStep::Criteria => SetupStep::Rules,
            SetupStep::Rules => SetupStep::Who,
            SetupStep::Who | SetupStep::Review => SetupStep::Review,
        };
        Ok(SetupDraft {
            step,
            ..self.clone()
        })
    }

    pub fn back(&self) -> SetupDraft {
        let first = match self.purpose {
            Purpose::Start => SetupStep::Basics,
            Purpose::Edit { .. } => SetupStep::Criteria,
        };
        let step = match self.step {
            SetupStep::Basics | SetupStep::Criteria => first,
            SetupStep::Rules => SetupStep::Criteria,
            SetupStep::Who => SetupStep::Rules,
            SetupStep::Review => SetupStep::Who,
        };
        SetupDraft {
            step,
            ..self.clone()
        }
    }

    pub fn ready(&self) -> Result<SetupReady, SetupRefusal> {
        let unmet = self.steps().into_iter().find(|step| !self.step_met(*step));
        if let Some(step) = unmet {
            return Err(SetupRefusal::StepIncomplete(step));
        }
        match self.purpose {
            Purpose::Start => {
                let (first_day, last_day) = self
                    .first_day
                    .zip(self.last_day)
                    .ok_or(SetupRefusal::StepIncomplete(SetupStep::Basics))?;
                Ok(SetupReady::Start(SetupPlan {
                    guild: self.guild,
                    name: self.name.clone(),
                    first_day,
                    last_day,
                    zone: self.zone.clone(),
                    criteria: self.criteria.clone(),
                    rules: self.rules.clone(),
                    roles: self.roles.clone(),
                    judges: self.judges.clone(),
                    marks: self.marks.clone(),
                    created_by: self.owner,
                }))
            }
            Purpose::Edit { competition } => Ok(SetupReady::Edit(ConfigChange {
                competition,
                criteria: self.criteria.clone(),
                rules: self.rules.clone(),
                roles: self.roles.clone(),
                judges: self.judges.clone(),
                marks: self.marks.clone(),
            })),
        }
    }

    pub fn panel(&self, panel: u64, _now_unix: i64, checks: &SetupChecks) -> PanelView {
        let (content, components) = match self.step {
            SetupStep::Basics => (self.basics_content(), self.basics_rows(panel)),
            SetupStep::Criteria => (self.criteria_content(), self.criteria_rows(panel)),
            SetupStep::Rules => (self.rules_content(), self.rules_rows(panel)),
            SetupStep::Who => (self.who_content(), self.who_rows(panel)),
            SetupStep::Review => (self.review_content(checks), self.review_rows(panel, checks)),
        };
        PanelView {
            content,
            components,
        }
    }

    fn starting(&self) -> Result<(), SetupRefusal> {
        match self.purpose {
            Purpose::Start => Ok(()),
            Purpose::Edit { .. } => Err(SetupRefusal::FixedAfterStart),
        }
    }

    fn steps(&self) -> Vec<SetupStep> {
        let later = [SetupStep::Criteria, SetupStep::Rules, SetupStep::Who];
        match self.purpose {
            Purpose::Start => std::iter::once(SetupStep::Basics).chain(later).collect(),
            Purpose::Edit { .. } => later.to_vec(),
        }
    }

    fn step_met(&self, step: SetupStep) -> bool {
        match step {
            SetupStep::Basics => self
                .first_day
                .zip(self.last_day)
                .is_some_and(|(first, last)| last > first),
            SetupStep::Criteria => !self.criteria.is_empty(),
            SetupStep::Rules => !self.rules.trim().is_empty(),
            SetupStep::Who => !self.roles.is_empty() && !self.judges.is_empty(),
            SetupStep::Review => true,
        }
    }

    fn editing(&self) -> bool {
        matches!(self.purpose, Purpose::Edit { .. })
    }

    fn top_lines(&self) -> Vec<String> {
        vec![
            iq_text::setup_heading(&self.name, self.editing()),
            iq_text::setup_step_line(self.step),
        ]
    }

    fn basics_content(&self) -> String {
        let dates = match self.first_day.zip(self.last_day) {
            Some((first, last)) => iq_text::basics_line(first, last),
            None => iq_text::BASICS_PROMPT.to_owned(),
        };
        let used = match self.template {
            Some(TemplateChoice::Competition(id)) => {
                self.templates.iter().find(|template| template.id == id)
            }
            Some(TemplateChoice::Blank) | None => None,
        };
        let template = match (self.templates.is_empty(), used) {
            (true, _) => iq_text::NO_TEMPLATES.to_owned(),
            (false, Some(source)) => iq_text::template_copied(&source.name),
            (false, None) => iq_text::TEMPLATES_OFFERED.to_owned(),
        };
        joined(self.top_lines().into_iter().chain([
            dates,
            iq_text::ends_line(&self.zone),
            template,
        ]))
    }

    fn criteria_content(&self) -> String {
        let fixed: Vec<String> = self
            .top_lines()
            .into_iter()
            .chain([iq_text::CRITERIA_INTRO.to_owned()])
            .collect();
        if self.criteria.is_empty() {
            return joined(fixed.into_iter().chain([iq_text::NO_CRITERIA.to_owned()]));
        }
        let lines: Vec<String> = self
            .criteria
            .iter()
            .enumerate()
            .map(|(index, criterion)| iq_text::criterion_line(index + 1, criterion))
            .collect();
        let whole = joined(fixed.iter().cloned().chain(lines.iter().cloned()));
        if whole.chars().count() <= CONTENT_LIMIT {
            return whole;
        }
        let shown = (0..lines.len())
            .rev()
            .find(|count| {
                let footer = iq_text::criteria_not_shown(lines.len() - count);
                let candidate = joined(
                    fixed
                        .iter()
                        .cloned()
                        .chain(lines.iter().take(*count).cloned())
                        .chain([footer]),
                );
                candidate.chars().count() <= CONTENT_LIMIT
            })
            .unwrap_or(0);
        joined(
            fixed
                .into_iter()
                .chain(lines.iter().take(shown).cloned())
                .chain([iq_text::criteria_not_shown(lines.len() - shown)]),
        )
    }

    fn rules_content(&self) -> String {
        let fixed: Vec<String> = self
            .top_lines()
            .into_iter()
            .chain([iq_text::RULES_INTRO.to_owned()])
            .collect();
        if self.rules.trim().is_empty() {
            return joined(fixed.into_iter().chain([iq_text::NO_RULES.to_owned()]));
        }
        let whole = joined(fixed.iter().cloned().chain([quoted(&self.rules)]));
        if whole.chars().count() <= CONTENT_LIMIT {
            return whole;
        }
        let total = self.rules.chars().count();
        let worst_footer = iq_text::rules_not_shown(total);
        let used = joined(fixed.iter().cloned().chain([String::new(), worst_footer]))
            .chars()
            .count();
        let budget = CONTENT_LIMIT.saturating_sub(used);
        let shown = quoted_prefix(&self.rules, budget);
        let shown_count = shown.chars().count();
        joined(fixed.into_iter().chain([
            quoted(&shown),
            iq_text::rules_not_shown(total - shown_count),
        ]))
    }

    fn who_content(&self) -> String {
        joined(self.top_lines().into_iter().chain([
            iq_text::WHO_ROLES_INTRO.to_owned(),
            iq_text::WHO_JUDGES_INTRO.to_owned(),
        ]))
    }

    fn review_content(&self, checks: &SetupChecks) -> String {
        let full = self.review_text(
            checks,
            &iq_text::role_list(&self.roles),
            &iq_text::judge_list(&self.judges),
        );
        if full.chars().count() <= CONTENT_LIMIT {
            return full;
        }
        self.review_text(
            checks,
            &iq_text::role_count(self.roles.len()),
            &iq_text::judge_count(self.judges.len()),
        )
    }

    fn review_text(&self, checks: &SetupChecks, roles: &str, judges: &str) -> String {
        let runs = self.first_day.zip(self.last_day).map(|(first, last)| {
            iq_text::review_runs(first, last, &self.zone, self.criteria.len())
        });
        let tree = match self.purpose {
            Purpose::Start => vec![
                iq_text::STARTING_CREATES.to_owned(),
                iq_text::category_branch(&self.name),
                iq_text::announcements_branch(roles),
                iq_text::SUBMISSIONS_BRANCH.to_owned(),
            ],
            Purpose::Edit { .. } => Vec::new(),
        };
        let role_line = checks
            .role_below_judge
            .then(|| iq_text::ROLE_BELOW_JUDGE.to_owned());
        let missing_line =
            (!checks.missing.is_empty()).then(|| iq_text::MISSING_SETUP_PERMISSIONS.to_owned());
        joined(
            self.top_lines()
                .into_iter()
                .chain(runs)
                .chain([iq_text::review_who(roles, judges, &self.marks)])
                .chain(tree)
                .chain(role_line)
                .chain(missing_line),
        )
    }

    fn basics_rows(&self, panel: u64) -> Vec<serenity::CreateActionRow> {
        let blank = serenity::CreateSelectMenuOption::new(iq_text::BLANK_LABEL, BLANK_TEMPLATE)
            .description(iq_text::BLANK_DESCRIPTION)
            .default_selection(self.template == Some(TemplateChoice::Blank));
        let templates = self.templates.iter().map(|template| {
            serenity::CreateSelectMenuOption::new(template.name.clone(), template.id.to_string())
                .description(iq_text::template_description(
                    template.first_day,
                    template.last_day,
                    template.criteria.len(),
                ))
                .default_selection(self.template == Some(TemplateChoice::Competition(template.id)))
        });
        let options = std::iter::once(blank).chain(templates).collect();
        vec![
            string_menu(
                panel,
                IqPanelPart::Template,
                iq_text::TEMPLATE_PLACEHOLDER,
                options,
            ),
            serenity::CreateActionRow::Buttons(vec![
                button(panel, IqPanelPart::SetDates, iq_text::SET_DATES_LABEL),
                button(panel, IqPanelPart::Rename, iq_text::RENAME_LABEL),
                next_button(
                    panel,
                    iq_text::NEXT_CRITERIA_LABEL,
                    !self.step_met(SetupStep::Basics),
                ),
                cancel_button(panel),
            ]),
        ]
    }

    fn criteria_rows(&self, panel: u64) -> Vec<serenity::CreateActionRow> {
        let menu = (!self.criteria.is_empty()).then(|| {
            string_menu(
                panel,
                IqPanelPart::Criterion,
                iq_text::CRITERION_PLACEHOLDER,
                self.criteria
                    .iter()
                    .enumerate()
                    .map(|(index, criterion)| {
                        serenity::CreateSelectMenuOption::new(
                            criterion.name.clone(),
                            index.to_string(),
                        )
                        .description(iq_text::criterion_option_description(criterion))
                        .default_selection(self.picked == Some(index))
                    })
                    .collect(),
            )
        });
        let nothing_picked = self.picked.is_none();
        let editing_row = serenity::CreateActionRow::Buttons(vec![
            button(panel, IqPanelPart::Add, iq_text::ADD_CRITERION_LABEL)
                .disabled(self.criteria.len() >= CRITERIA_LIMIT),
            button(panel, IqPanelPart::Edit, iq_text::EDIT_PICKED_LABEL).disabled(nothing_picked),
            button(panel, IqPanelPart::Remove, iq_text::REMOVE_PICKED_LABEL)
                .disabled(nothing_picked),
        ]);
        let moving_row = serenity::CreateActionRow::Buttons(vec![
            back_button(panel),
            next_button(
                panel,
                iq_text::NEXT_RULES_LABEL,
                !self.step_met(SetupStep::Criteria),
            ),
            cancel_button(panel),
        ]);
        menu.into_iter().chain([editing_row, moving_row]).collect()
    }

    fn rules_rows(&self, panel: u64) -> Vec<serenity::CreateActionRow> {
        vec![
            serenity::CreateActionRow::Buttons(vec![button(
                panel,
                IqPanelPart::Rules,
                iq_text::WRITE_RULES_LABEL,
            )]),
            serenity::CreateActionRow::Buttons(vec![
                back_button(panel),
                next_button(
                    panel,
                    iq_text::NEXT_WHO_LABEL,
                    !self.step_met(SetupStep::Rules),
                ),
                cancel_button(panel),
            ]),
        ]
    }

    fn who_rows(&self, panel: u64) -> Vec<serenity::CreateActionRow> {
        let roles = serenity::CreateSelectMenu::new(
            iq_panel_id(panel, IqPanelPart::Roles),
            serenity::CreateSelectMenuKind::Role {
                default_roles: (!self.roles.is_empty()).then(|| {
                    self.roles
                        .iter()
                        .filter_map(|role| NonZeroU64::new(role.get()).map(serenity::RoleId::from))
                        .collect()
                }),
            },
        )
        .placeholder(iq_text::ROLES_PLACEHOLDER)
        .min_values(1)
        .max_values(MENU_MAX);
        let judges = serenity::CreateSelectMenu::new(
            iq_panel_id(panel, IqPanelPart::Judges),
            serenity::CreateSelectMenuKind::User {
                default_users: (!self.judges.is_empty()).then(|| {
                    self.judges
                        .iter()
                        .filter_map(|judge| {
                            NonZeroU64::new(judge.get()).map(serenity::UserId::from)
                        })
                        .collect()
                }),
            },
        )
        .placeholder(iq_text::JUDGES_PLACEHOLDER)
        .min_values(1)
        .max_values(MENU_MAX);
        let reactions = AWARDED_CHOICES
            .into_iter()
            .filter_map(Mark::parse)
            .map(|choice| {
                serenity::CreateSelectMenuOption::new(iq_text::mark_label(&choice), choice.text())
                    .default_selection(self.marks.awarded == choice)
            })
            .collect();
        vec![
            serenity::CreateActionRow::SelectMenu(roles),
            serenity::CreateActionRow::SelectMenu(judges),
            string_menu(
                panel,
                IqPanelPart::Reaction,
                iq_text::REACTION_PLACEHOLDER,
                reactions,
            ),
            serenity::CreateActionRow::Buttons(vec![
                back_button(panel),
                next_button(
                    panel,
                    iq_text::NEXT_REVIEW_LABEL,
                    !self.step_met(SetupStep::Who),
                ),
                cancel_button(panel),
            ]),
        ]
    }

    fn review_rows(&self, panel: u64, checks: &SetupChecks) -> Vec<serenity::CreateActionRow> {
        let label = match self.purpose {
            Purpose::Start => iq_text::START_LABEL,
            Purpose::Edit { .. } => iq_text::SAVE_CHANGES_LABEL,
        };
        let blocked = !checks.pass() || self.ready().is_err();
        vec![serenity::CreateActionRow::Buttons(vec![
            back_button(panel),
            serenity::CreateButton::new(iq_panel_id(panel, IqPanelPart::Start))
                .label(label)
                .style(serenity::ButtonStyle::Success)
                .disabled(blocked),
            cancel_button(panel),
        ])]
    }
}

pub fn rename_modal(panel: u64, name: &str) -> serenity::CreateModal {
    serenity::CreateModal::new(
        iq_panel_id(panel, IqPanelPart::RenameModal),
        iq_text::RENAME_MODAL_TITLE,
    )
    .components(vec![serenity::CreateActionRow::InputText(text_input(
        serenity::InputTextStyle::Short,
        iq_text::RENAME_INPUT_LABEL,
        NAME_INPUT,
        NAME_LIMIT,
        name,
    ))])
}

pub fn criterion_modal(panel: u64, editing: Option<&Criterion>) -> serenity::CreateModal {
    let title = match editing {
        Some(_) => iq_text::EDIT_CRITERION_TITLE,
        None => iq_text::ADD_CRITERION_TITLE,
    };
    let points = editing.map_or(String::new(), |criterion| criterion.points.to_string());
    let inputs = [
        text_input(
            serenity::InputTextStyle::Short,
            iq_text::CRITERION_NAME_LABEL,
            NAME_INPUT,
            CRITERION_NAME_LIMIT,
            editing.map_or("", |criterion| criterion.name.as_str()),
        ),
        text_input(
            serenity::InputTextStyle::Paragraph,
            iq_text::CRITERION_DESCRIPTION_LABEL,
            DESCRIPTION_INPUT,
            DESCRIPTION_LIMIT,
            editing.map_or("", |criterion| criterion.description.as_str()),
        )
        .required(false),
        text_input(
            serenity::InputTextStyle::Short,
            iq_text::CRITERION_POINTS_LABEL,
            POINTS_INPUT,
            POINTS_INPUT_MAX,
            &points,
        ),
    ];
    serenity::CreateModal::new(iq_panel_id(panel, IqPanelPart::CriterionModal), title).components(
        inputs
            .into_iter()
            .map(serenity::CreateActionRow::InputText)
            .collect(),
    )
}

pub fn rules_modal(panel: u64, rules: &str) -> serenity::CreateModal {
    serenity::CreateModal::new(
        iq_panel_id(panel, IqPanelPart::RulesModal),
        iq_text::RULES_MODAL_TITLE,
    )
    .components(vec![serenity::CreateActionRow::InputText(text_input(
        serenity::InputTextStyle::Paragraph,
        iq_text::RULES_INPUT_LABEL,
        RULES_INPUT,
        RULES_LIMIT,
        rules,
    ))])
}

pub fn dates_modal(panel: u64) -> serenity::CreateModal {
    serenity::CreateModal::new(
        iq_panel_id(panel, IqPanelPart::DatesModal),
        iq_text::DATES_MODAL_TITLE,
    )
    .components(vec![
        day_input(
            iq_text::FIRST_DAY_INPUT_LABEL,
            FIRST_DAY_INPUT,
            iq_text::FIRST_DAY_EXAMPLE,
        ),
        day_input(
            iq_text::LAST_DAY_INPUT_LABEL,
            LAST_DAY_INPUT,
            iq_text::LAST_DAY_EXAMPLE,
        ),
    ])
}

fn day_input(label: &str, custom_id: &str, example: &str) -> serenity::CreateActionRow {
    serenity::CreateActionRow::InputText(
        serenity::CreateInputText::new(serenity::InputTextStyle::Short, label, custom_id)
            .placeholder(example)
            .max_length(DAY_INPUT_MAX),
    )
}

fn text_input(
    style: serenity::InputTextStyle,
    label: &str,
    custom_id: &str,
    max_length: usize,
    value: &str,
) -> serenity::CreateInputText {
    let input = serenity::CreateInputText::new(style, label, custom_id)
        .max_length(u16::try_from(max_length).unwrap_or(u16::MAX));
    match value.is_empty() {
        true => input,
        false => input.value(value),
    }
}

fn string_menu(
    panel: u64,
    part: IqPanelPart,
    placeholder: &str,
    options: Vec<serenity::CreateSelectMenuOption>,
) -> serenity::CreateActionRow {
    serenity::CreateActionRow::SelectMenu(
        serenity::CreateSelectMenu::new(
            iq_panel_id(panel, part),
            serenity::CreateSelectMenuKind::String { options },
        )
        .placeholder(placeholder),
    )
}

fn button(panel: u64, part: IqPanelPart, label: &str) -> serenity::CreateButton {
    serenity::CreateButton::new(iq_panel_id(panel, part))
        .label(label)
        .style(serenity::ButtonStyle::Secondary)
}

fn next_button(panel: u64, label: &str, blocked: bool) -> serenity::CreateButton {
    serenity::CreateButton::new(iq_panel_id(panel, IqPanelPart::Next))
        .label(label)
        .style(serenity::ButtonStyle::Primary)
        .disabled(blocked)
}

fn back_button(panel: u64) -> serenity::CreateButton {
    button(panel, IqPanelPart::Back, iq_text::BACK_LABEL)
}

fn cancel_button(panel: u64) -> serenity::CreateButton {
    button(panel, IqPanelPart::Cancel, iq_text::CANCEL_LABEL)
}

fn start_of(day: Date) -> Option<i64> {
    day.to_zoned(TimeZone::UTC)
        .ok()
        .map(|midnight| midnight.timestamp().as_second())
}

fn read_points(text: &str) -> Option<u32> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<u32>()
        .ok()
        .filter(|points| (1..=POINTS_LIMIT).contains(points))
}

fn quoted(text: &str) -> String {
    text.split('\n')
        .map(|line| format!("{QUOTE_MARK}{line}"))
        .collect::<Vec<String>>()
        .join("\n")
}

fn quoted_prefix(text: &str, budget: usize) -> String {
    let fitting = text
        .chars()
        .scan(QUOTE_MARK.len(), |size, character| {
            *size += match character {
                '\n' => 1 + QUOTE_MARK.len(),
                _ => 1,
            };
            Some(*size)
        })
        .take_while(|size| *size <= budget)
        .count();
    text.chars().take(fitting).collect()
}

fn joined(lines: impl Iterator<Item = String>) -> String {
    lines.collect::<Vec<String>>().join("\n")
}

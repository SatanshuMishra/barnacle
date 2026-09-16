use barnacle_catalog::Catalog;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::curation::Curated;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::Problem;
use barnacle_catalog::curation::curate;
use barnacle_catalog::curation::validate;
use barnacle_catalog::diff::diff;
use barnacle_catalog::diff::year_refit_candidates;

fn label(catalog: &Catalog) -> String {
    format!(
        "{} (build {})",
        catalog.provenance.game_version, catalog.provenance.build
    )
}

fn ship_line(catalog: &Catalog, curated: &Curated, index: &ShipIndex) -> String {
    let Some(ship) = catalog.get(index) else {
        return format!("  {index}");
    };
    let name = ship
        .name
        .as_ref()
        .map(|name| name.display())
        .unwrap_or("(no English name)");
    let outcome = match curated.removed.get(index) {
        Some(removal) => format!("removed: {removal}"),
        None => "in pool".to_owned(),
    };
    format!(
        "  {index}  {name}  tier {}  {}  -> {outcome}",
        ship.tier.get(),
        ship.group
    )
}

fn name_of(catalog: &Catalog, index: &ShipIndex) -> String {
    catalog
        .get(index)
        .and_then(|ship| ship.name.as_ref())
        .map(|name| format!("{} ({index})", name.display()))
        .unwrap_or_else(|| index.to_string())
}

pub fn diff_report(old: Option<&Catalog>, new: &Catalog, config: &CurationConfig) -> String {
    let changes = diff(old, new);
    let curated = curate(new, config);
    let problems = validate(new, config, &curated);
    let candidates = year_refit_candidates(new, config, &curated);
    let compared = old
        .map(label)
        .unwrap_or_else(|| "nothing (first catalog)".to_owned());
    let groups = changes
        .new_groups
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    let header = vec![
        format!("Catalog {}, compared with {compared}", label(new)),
        format!("Pool: {} ships", curated.pool.len()),
        format!(
            "New ship groups: {}",
            if groups.is_empty() {
                "none".to_owned()
            } else {
                groups
            }
        ),
    ];
    let regrouped = std::iter::once(format!("Regrouped ({})", changes.regrouped.len())).chain(
        changes
            .regrouped
            .iter()
            .map(|change| format!("  {}  {} -> {}", change.index, change.from, change.to)),
    );
    let added = std::iter::once(format!("Added ({})", changes.added.len())).chain(
        changes
            .added
            .iter()
            .map(|index| ship_line(new, &curated, index)),
    );
    let removed = std::iter::once(format!("Removed ({})", changes.removed.len()))
        .chain(changes.removed.iter().map(|index| format!("  {index}")));
    let lookalikes = std::iter::once(format!("Lookalike candidates ({})", candidates.len())).chain(
        candidates.iter().map(|pair| {
            format!(
                "  {} and {}",
                name_of(new, &pair.original),
                name_of(new, &pair.refit)
            )
        }),
    );
    let problem_lines: Vec<String> = if problems.is_empty() {
        vec!["Curation problems: none".to_owned()]
    } else {
        std::iter::once(format!("Curation problems ({})", problems.len()))
            .chain(problems.iter().map(|problem| format!("  - {problem}")))
            .collect()
    };

    header
        .into_iter()
        .chain(regrouped)
        .chain(added)
        .chain(removed)
        .chain(lookalikes)
        .chain(problem_lines)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn problems_report(name: &str, problems: &[Problem]) -> String {
    if problems.is_empty() {
        return format!("{name}: no curation problems");
    }
    std::iter::once(format!("{name}: {} curation problem(s)", problems.len()))
        .chain(problems.iter().map(|problem| format!("  - {problem}")))
        .collect::<Vec<_>>()
        .join("\n")
}

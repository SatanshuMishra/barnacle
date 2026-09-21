use barnacle_catalog::Catalog;
use barnacle_catalog::Nation;
use barnacle_catalog::ParamId;
use barnacle_catalog::Provenance;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipGroup;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::ShipName;
use barnacle_catalog::Silhouette;
use barnacle_catalog::Tier;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::Problem;
use barnacle_data::report::diff_report;
use barnacle_data::report::problems_report;

fn ship(index: &str, name: &str, group: &str, tier: u32, hash: &str) -> Ship {
    Ship {
        id: ParamId::new(1),
        index: ShipIndex::parse(index).unwrap(),
        tier: Tier::new(tier).unwrap(),
        group: ShipGroup::new(group),
        class: ShipClass::Cruiser,
        nation: Nation::new("Germany"),
        is_paper: false,
        name: Some(ShipName {
            short: name.to_owned(),
            full: Some(name.to_owned()),
        }),
        silhouette: Some(Silhouette {
            sha256: hash.to_owned(),
        }),
        hull_model: None,
    }
}

fn catalog(version: &str, build: u32, ships: Vec<Ship>) -> Catalog {
    Catalog {
        provenance: Provenance {
            game_version: version.to_owned(),
            build,
            data_repo_commit: "0000000".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        ships,
    }
}

#[test]
fn diff_report_shows_each_new_ship_and_what_curation_did() {
    let old = catalog(
        "15.7.0",
        13015811,
        vec![ship("PGSC519", "Ägir", "special", 9, "5fe2")],
    );
    let new = catalog(
        "15.8.0",
        13187581,
        vec![
            ship("PGSC519", "Ägir", "special", 9, "5fe2"),
            ship("PGSC899", "AL Ägir", "special", 9, "5fe2"),
        ],
    );
    let config =
        CurationConfig::from_toml("reviewed_through = 13015811\ngroups = [\"special\"]").unwrap();
    let report = diff_report(Some(&old), &new, &config);
    assert!(
        report.contains("Catalog 15.8.0 (build 13187581), compared with 15.7.0 (build 13015811)"),
        "{report}"
    );
    assert!(report.contains("Added (1)"), "{report}");
    assert!(
        report
            .contains("PGSC899  AL Ägir  tier 9  special  -> removed: same silhouette as PGSC519"),
        "{report}"
    );
    assert!(
        report
            .contains("Removed (0)\nLeft the pool (0)\nJoined the pool (0)\nLookalike candidates"),
        "{report}"
    );
    assert!(
        report.contains(
            "curation was reviewed through build 13015811, but the catalog is build 13187581"
        ),
        "{report}"
    );
}

#[test]
fn a_first_diff_says_there_is_nothing_to_compare_with() {
    let new = catalog(
        "15.8.0",
        13187581,
        vec![ship("PGSC519", "Ägir", "special", 9, "5fe2")],
    );
    let config =
        CurationConfig::from_toml("reviewed_through = 13187581\ngroups = [\"special\"]").unwrap();
    let report = diff_report(None, &new, &config);
    assert!(
        report.contains("compared with nothing (first catalog)"),
        "{report}"
    );
    assert!(
        report.contains("PGSC519  Ägir  tier 9  special  -> in pool"),
        "{report}"
    );
    assert!(report.contains("New ship groups: special"), "{report}");
    assert!(
        report
            .contains("Removed (0)\nLeft the pool (0)\nJoined the pool (0)\nLookalike candidates"),
        "{report}"
    );
    assert!(report.contains("Curation problems: none"), "{report}");
}

fn hulled(ship: Ship, model: &str) -> Ship {
    Ship {
        hull_model: Some(model.to_owned()),
        ..ship
    }
}

const AMAGI_HULL: &str =
    "content/gameplay/japan/ship/battleship/JSB013_Amagi_1942/JSB013_Amagi_1942.model";

fn hull_config() -> CurationConfig {
    CurationConfig::from_toml(
        "reviewed_through = 13187581\ngroups = [\"upgradeable\", \"special\"]",
    )
    .unwrap()
}

#[test]
fn the_diff_lists_ships_that_left_or_joined_the_pool() {
    let old = catalog(
        "15.7.0",
        13015811,
        vec![
            ship("PJSB013", "Amagi", "upgradeable", 8, "a013"),
            ship("PJSB878", "Ignis Purgatio", "special", 8, "a878"),
        ],
    );
    let new = catalog(
        "15.8.0",
        13187581,
        vec![
            hulled(
                ship("PJSB013", "Amagi", "upgradeable", 8, "a013"),
                AMAGI_HULL,
            ),
            hulled(
                ship("PJSB878", "Ignis Purgatio", "special", 8, "a878"),
                AMAGI_HULL,
            ),
        ],
    );
    let report = diff_report(Some(&old), &new, &hull_config());
    assert!(
        report.contains(
            "Removed (0)\nLeft the pool (1)\n  PJSB878  Ignis Purgatio  tier 8  special  -> removed: same hull model as PJSB013\nJoined the pool (0)\nLookalike candidates"
        ),
        "{report}"
    );
}

#[test]
fn only_ships_in_both_catalogs_can_leave_or_join_the_pool() {
    let old = catalog(
        "15.7.0",
        13015811,
        vec![
            hulled(
                ship("PJSB013", "Amagi", "upgradeable", 8, "a013"),
                AMAGI_HULL,
            ),
            hulled(
                ship("PJSB878", "Ignis Purgatio", "special", 8, "a878"),
                AMAGI_HULL,
            ),
            ship("PGSC519", "Ägir", "special", 9, "5fe2"),
        ],
    );
    let new = catalog(
        "15.8.0",
        13187581,
        vec![
            ship("PJSB013", "Amagi", "upgradeable", 8, "a013"),
            ship("PJSB878", "Ignis Purgatio", "special", 8, "a878"),
            ship("PASB008", "Colorado", "upgradeable", 7, "a008"),
        ],
    );
    let report = diff_report(Some(&old), &new, &hull_config());
    assert!(
        report.contains(
            "Removed (1)\n  PGSC519\nLeft the pool (0)\nJoined the pool (1)\n  PJSB878  Ignis Purgatio  tier 8  special  -> in pool\nLookalike candidates"
        ),
        "{report}"
    );
}

#[test]
fn problems_report_lists_each_problem() {
    assert_eq!(
        problems_report("15.8.0_13187581", &[]),
        "15.8.0_13187581: no curation problems"
    );
    assert_eq!(
        problems_report(
            "15.8.0_13187581",
            &[Problem::NotReviewed { build: 13187581 }]
        ),
        "15.8.0_13187581: 1 curation problem(s)\n  - curation has never been reviewed; review build 13187581 and set reviewed_through"
    );
}

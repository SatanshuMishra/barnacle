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
    assert!(report.contains("Curation problems: none"), "{report}");
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

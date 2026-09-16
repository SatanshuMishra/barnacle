mod common;

use barnacle_catalog::Ship;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipGroup;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::curate;
use barnacle_catalog::diff::LookalikeCandidate;
use barnacle_catalog::diff::Regrouped;
use barnacle_catalog::diff::diff;
use barnacle_catalog::diff::year_refit_candidates;
use common::catalog;
use common::index;
use common::ship;

#[test]
fn diff_lists_added_removed_and_regrouped_ships() {
    let old = catalog(
        13015811,
        vec![
            ship("PASB008", "Colorado", "upgradeable", 7, "a1"),
            ship("PASS910", "Balao 2", "demoWithoutStatsPrem", 10, "a2"),
            ship("PBSC710", "Monmouth", "upgradeable", 7, "a3"),
        ],
    );
    let new = catalog(
        13187581,
        vec![
            ship("PASB008", "Colorado", "upgradeable", 7, "a1"),
            ship("PASS910", "Balao 2", "special", 10, "a2"),
            ship("PBSC210", "Goliath", "upgradeable", 10, "a4"),
        ],
    );
    let changes = diff(Some(&old), &new);
    assert_eq!(changes.added, vec![index("PBSC210")]);
    assert_eq!(changes.removed, vec![index("PBSC710")]);
    assert_eq!(
        changes.regrouped,
        vec![Regrouped {
            index: index("PASS910"),
            from: ShipGroup::new("demoWithoutStatsPrem"),
            to: ShipGroup::new("special"),
        }]
    );
    assert_eq!(changes.new_groups, vec![ShipGroup::new("special")]);
}

#[test]
fn a_first_diff_treats_every_ship_and_group_as_new() {
    let new = catalog(
        13187581,
        vec![ship("PASB008", "Colorado", "upgradeable", 7, "a1")],
    );
    let changes = diff(None, &new);
    assert_eq!(changes.added, vec![index("PASB008")]);
    assert_eq!(changes.new_groups, vec![ShipGroup::new("upgradeable")]);
}

#[test]
fn year_suffixed_refits_are_lookalike_candidates_until_grouped() {
    let ships = catalog(
        13187581,
        vec![
            ship("PBSC507", "Belfast", "special", 7, "c866"),
            ship("PBSC528", "Belfast '43", "special", 8, "61fc"),
            ship("PGSD710", "Georg Hoffmann", "ultimate", 10, "1e02"),
        ],
    );
    let groups = "groups = [\"special\", \"ultimate\"]";
    let ungrouped = CurationConfig::from_toml(groups).unwrap();
    assert_eq!(
        year_refit_candidates(&ships, &ungrouped, &curate(&ships, &ungrouped)),
        vec![LookalikeCandidate {
            original: index("PBSC507"),
            refit: index("PBSC528")
        }]
    );
    let grouped = CurationConfig::from_toml(&format!(
        "{groups}\n[[lookalikes]]\nships = [\"PBSC507\", \"PBSC528\"]"
    ))
    .unwrap();
    assert!(year_refit_candidates(&ships, &grouped, &curate(&ships, &grouped)).is_empty());
}

#[test]
fn a_year_suffix_on_a_different_class_of_ship_is_not_a_lookalike() {
    let battleship = |value, name, tier, hash| Ship {
        class: ShipClass::Battleship,
        ..ship(value, name, "special", tier, hash)
    };
    let ships = catalog(
        13187581,
        vec![
            battleship("PBSB205", "Tiger", 5, "t1"),
            ship("PBSC518", "Tiger '59", "special", 8, "t2"),
            battleship("PBSB104", "Orion", 4, "o1"),
            ship("PBSC716", "Orion '44", "special", 7, "o2"),
        ],
    );
    let config = CurationConfig::from_toml("groups = [\"special\"]").unwrap();
    assert!(year_refit_candidates(&ships, &config, &curate(&ships, &config)).is_empty());
}

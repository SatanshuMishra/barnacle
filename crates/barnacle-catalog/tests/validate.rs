mod common;

use barnacle_catalog::Ship;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::Problem;
use barnacle_catalog::curation::Section;
use barnacle_catalog::curation::curate;
use barnacle_catalog::curation::validate;
use common::catalog;
use common::index;
use common::ship;

const BUILD: u32 = 13187581;

fn problems(ships: Vec<Ship>, text: &str) -> Vec<Problem> {
    let catalog = catalog(BUILD, ships);
    let config = CurationConfig::from_toml(text).unwrap();
    let curated = curate(&catalog, &config);
    validate(&catalog, &config, &curated)
}

fn reviewed(tables: &str) -> String {
    format!("reviewed_through = {BUILD}\ngroups = [\"special\", \"upgradeable\"]\n{tables}")
}

#[test]
fn a_reviewed_config_with_resolvable_entries_has_no_problems() {
    let ships = vec![
        ship("PBSC507", "Belfast", "special", 7, "c866"),
        ship("PBSC528", "Belfast '43", "special", 8, "61fc"),
    ];
    assert_eq!(
        problems(
            ships,
            &reviewed("[[lookalikes]]\nships = [\"PBSC507\", \"PBSC528\"]")
        ),
        vec![]
    );
}

#[test]
fn a_lookalike_naming_a_missing_ship_is_reported() {
    let ships = vec![ship("PBSC210", "Goliath", "upgradeable", 10, "b1")];
    let found = problems(
        ships,
        &reviewed("[[lookalikes]]\nships = [\"PBSC210\", \"PBSC710\"]"),
    );
    assert_eq!(
        found,
        vec![
            Problem::UnknownIndex {
                section: Section::Lookalikes,
                index: index("PBSC710")
            },
            Problem::LookalikeGroupTooSmall {
                position: 1,
                eligible: 1
            },
        ]
    );
}

#[test]
fn a_ship_excluded_twice_is_reported() {
    let ships = vec![ship("PASA898", "AL Hornet", "special", 8, "c1")];
    let tables = "[[exclude]]\nindex = \"PASA898\"\nreason = \"carbon_copy\"\n\n[[exclude]]\nindex = \"PASA898\"\nreason = \"carbon_copy\"";
    assert_eq!(
        problems(ships, &reviewed(tables)),
        vec![Problem::DuplicateExclude {
            index: index("PASA898")
        }]
    );
}

#[test]
fn excluded_and_kept_and_unknown_bases_are_reported() {
    let ships = vec![ship("PJSC708", "ARP Takao", "special", 8, "91e9")];
    let tables = "[[exclude]]\nindex = \"PJSC708\"\nreason = \"carbon_copy\"\nbase = \"PJSC038\"\n\n[[keep]]\nindex = \"PJSC708\"";
    assert_eq!(
        problems(ships, &reviewed(tables)),
        vec![
            Problem::UnknownIndex {
                section: Section::ExcludeBase,
                index: index("PJSC038")
            },
            Problem::ExcludedAndKept {
                index: index("PJSC708")
            },
        ]
    );
}

#[test]
fn lookalike_members_must_be_in_the_pool_and_in_one_group() {
    let ships = vec![
        ship("PGSC519", "Ägir", "special", 9, "5fe2"),
        ship("PGSC899", "AL Ägir", "special", 9, "5fe2"),
        ship("PBSC507", "Belfast", "special", 7, "c866"),
    ];
    let tables = "[[lookalikes]]\nships = [\"PGSC519\", \"PGSC899\"]\n\n[[lookalikes]]\nships = [\"PGSC519\", \"PBSC507\"]";
    assert_eq!(
        problems(ships, &reviewed(tables)),
        vec![
            Problem::InSeveralLookalikeGroups {
                index: index("PGSC519")
            },
            Problem::LookalikeNotInPool {
                index: index("PGSC899")
            },
            Problem::LookalikeGroupTooSmall {
                position: 1,
                eligible: 1
            },
        ]
    );
}

#[test]
fn curation_must_be_reviewed_for_the_catalog_build() {
    let ships = vec![ship("PBSC507", "Belfast", "special", 7, "c866")];
    assert_eq!(
        problems(ships.clone(), "groups = [\"special\"]"),
        vec![Problem::NotReviewed { build: BUILD }]
    );
    assert_eq!(
        problems(ships, "reviewed_through = 13015811\ngroups = [\"special\"]"),
        vec![Problem::ReviewBehind {
            reviewed_through: 13015811,
            build: BUILD
        }]
    );
}

#[test]
fn problems_read_as_sentences() {
    assert_eq!(
        Problem::UnknownIndex {
            section: Section::Lookalikes,
            index: index("PBSC710")
        }
        .to_string(),
        "lookalikes names PBSC710, which is not in the catalog"
    );
    assert_eq!(
        Problem::NotReviewed { build: BUILD }.to_string(),
        "curation has never been reviewed; review build 13187581 and set reviewed_through"
    );
}

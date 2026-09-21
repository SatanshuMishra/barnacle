mod common;

use barnacle_catalog::Ship;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::ExcludeReason;
use barnacle_catalog::curation::Problem;
use barnacle_catalog::curation::Removal;
use barnacle_catalog::curation::curate;
use barnacle_catalog::curation::validate;
use common::catalog;
use common::index;
use common::ship;

const GROUPS: &str = r#"
groups = ["start", "special", "premium", "specialUnsellable", "ultimate", "upgradeable", "upgradeableExclusive", "upgradeableUltimate", "superShip"]
"#;

const AMAGI_HULL: &str =
    "content/gameplay/japan/ship/battleship/JSB013_Amagi_1942/JSB013_Amagi_1942.model";

fn config(tables: &str) -> CurationConfig {
    CurationConfig::from_toml(&format!("{GROUPS}\n{tables}")).unwrap()
}

fn hulled(ship: Ship, model: &str) -> Ship {
    Ship {
        hull_model: Some(model.to_owned()),
        ..ship
    }
}

#[test]
fn config_parses_every_section() {
    let parsed = config(
        r#"
[[exclude]]
index = "PJSC708"
reason = "carbon_copy"
base = "PJSC038"

[[keep]]
index = "PGSD720"

[[lookalikes]]
ships = ["PBSC507", "PBSC528"]

[[aliases]]
index = "PRSB110"
names = ["kreml"]
"#,
    );
    assert_eq!(parsed.reviewed_through, None);
    assert_eq!(parsed.groups.len(), 9);
    assert_eq!(parsed.exclude[0].reason, ExcludeReason::CarbonCopy);
    assert_eq!(parsed.exclude[0].base, Some(index("PJSC038")));
    assert_eq!(parsed.keep[0].index, index("PGSD720"));
    assert_eq!(
        parsed.lookalikes[0].ships,
        vec![index("PBSC507"), index("PBSC528")]
    );
    assert_eq!(parsed.aliases[0].names, vec!["kreml".to_owned()]);
}

#[test]
fn config_rejects_unknown_keys_and_bad_indices() {
    assert!(CurationConfig::from_toml(&format!("{GROUPS}\nforbidden = []")).is_err());
    assert!(CurationConfig::from_toml(&format!("{GROUPS}\n[[keep]]\nindex = \"bad\"")).is_err());
}

#[test]
fn ships_outside_the_allowed_groups_are_removed() {
    let ships = catalog(
        1,
        vec![ship("PASS910", "Balao 2", "demoWithoutStatsPrem", 10, "a1")],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(curated.removed[&index("PASS910")], Removal::GroupNotAllowed);
    assert!(curated.pool.is_empty());
}

#[test]
fn ships_without_a_name_or_silhouette_are_removed() {
    let unnamed = Ship {
        name: None,
        ..ship("PASB008", "Colorado", "upgradeable", 7, "a1")
    };
    let unseen = Ship {
        silhouette: None,
        ..ship("PASB018", "Iowa", "upgradeable", 9, "a2")
    };
    let curated = curate(&catalog(1, vec![unnamed, unseen]), &config(""));
    assert_eq!(curated.removed[&index("PASB008")], Removal::NoEnglishName);
    assert_eq!(curated.removed[&index("PASB018")], Removal::NoSilhouette);
}

#[test]
fn identical_silhouettes_keep_the_tech_tree_ship() {
    let ships = catalog(
        1,
        vec![
            ship("PASD019", "Clemson", "upgradeable", 4, "db18"),
            ship("PASD704", "DD 214", "specialUnsellable", 4, "db18"),
            ship("PGSC519", "Ägir", "special", 9, "5fe2"),
            ship("PGSC899", "AL Ägir", "special", 9, "5fe2"),
            ship("PGSD710", "Georg Hoffmann", "ultimate", 10, "1e02"),
            ship("PGSD720", "Georg Hoffmann Golden", "ultimate", 10, "1e02"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(
        curated.removed[&index("PASD704")],
        Removal::IdenticalSilhouette {
            base: index("PASD019")
        }
    );
    assert_eq!(
        curated.removed[&index("PGSC899")],
        Removal::IdenticalSilhouette {
            base: index("PGSC519")
        }
    );
    assert_eq!(
        curated.removed[&index("PGSD720")],
        Removal::IdenticalSilhouette {
            base: index("PGSD710")
        }
    );
    assert_eq!(
        curated.pool.iter().map(|i| i.as_str()).collect::<Vec<_>>(),
        ["PASD019", "PGSC519", "PGSD710"]
    );
    assert_eq!(
        curated.variants_of(&index("PGSC519")),
        vec![&index("PGSC899")]
    );
}

#[test]
fn identical_silhouettes_never_prefer_a_reskin_name() {
    let ships = catalog(
        1,
        vec![
            ship("PJSD596", "Shinonome B", "special", 6, "4a1c"),
            ship("PJSD706", "Shinonome", "special", 6, "4a1c"),
            ship("PFSB518", "Jean Bart", "specialUnsellable", 8, "9b0e"),
            ship("PFSB599", "Jean Bart B", "special", 8, "9b0e"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(
        curated.removed[&index("PJSD596")],
        Removal::IdenticalSilhouette {
            base: index("PJSD706")
        }
    );
    assert_eq!(
        curated.removed[&index("PFSB599")],
        Removal::IdenticalSilhouette {
            base: index("PFSB518")
        }
    );
}

#[test]
fn collaboration_reskins_are_removed_even_with_a_unique_silhouette() {
    let ships = catalog(
        1,
        vec![
            ship("PJSC038", "Atago", "special", 8, "57cd"),
            ship("PJSC708", "ARP Takao", "special", 8, "91e9"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(
        curated.removed[&index("PJSC708")],
        Removal::CollaborationPrefix
    );
    assert!(curated.pool.contains(&index("PJSC038")));
}

#[test]
fn variant_suffixes_point_at_the_ship_they_copy() {
    let ships = catalog(
        1,
        vec![
            ship("PASB518", "Massachusetts", "special", 8, "23ba"),
            ship("PASB598", "Massachusetts B", "special", 8, "ebd0"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(
        curated.removed[&index("PASB598")],
        Removal::VariantSuffix {
            base: index("PASB518")
        }
    );
}

#[test]
fn different_ships_that_share_a_word_stay_in_the_pool() {
    let ships = catalog(
        1,
        vec![
            ship("PASD709", "Black", "special", 9, "c8f8"),
            ship("PBSC101", "Black Swan", "start", 1, "d313"),
            ship("PGSB105", "König", "upgradeable", 5, "66d7"),
            ship("PGSB503", "König Albert", "special", 3, "2538"),
            ship("PUSD503", "Vampire", "special", 3, "da5f"),
            ship("PUSD510", "Vampire II", "ultimate", 10, "d6f0"),
            ship("PBSC507", "Belfast", "special", 7, "c866"),
            ship("PBSC528", "Belfast '43", "special", 8, "61fc"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert!(curated.removed.is_empty());
    assert_eq!(curated.pool.len(), 8);
}

#[test]
fn keep_protects_a_ship_from_the_automatic_rules() {
    let ships = catalog(
        1,
        vec![
            ship("PGSD710", "Georg Hoffmann", "ultimate", 10, "1e02"),
            ship("PGSD720", "Georg Hoffmann Golden", "ultimate", 10, "1e02"),
        ],
    );
    let curated = curate(&ships, &config("[[keep]]\nindex = \"PGSD720\""));
    assert!(curated.removed.is_empty());
    assert!(curated.pool.contains(&index("PGSD720")));
}

#[test]
fn manual_exclusions_replace_automatic_reasons_but_not_group_removals() {
    let ships = catalog(
        1,
        vec![
            ship("PJSC038", "Atago", "special", 8, "57cd"),
            ship("PJSC708", "ARP Takao", "special", 8, "91e9"),
            ship("PASS910", "Balao 2", "demoWithoutStatsPrem", 10, "7640"),
        ],
    );
    let tables = r#"
[[exclude]]
index = "PJSC708"
reason = "carbon_copy"
base = "PJSC038"

[[exclude]]
index = "PASS910"
reason = "bad_silhouette"
"#;
    let curated = curate(&ships, &config(tables));
    assert_eq!(
        curated.removed[&index("PJSC708")],
        Removal::Manual {
            reason: ExcludeReason::CarbonCopy,
            base: Some(index("PJSC038"))
        }
    );
    assert_eq!(curated.removed[&index("PASS910")], Removal::GroupNotAllowed);
    assert_eq!(
        curated.variants_of(&index("PJSC038")),
        vec![&index("PJSC708")]
    );
}

#[test]
fn removals_explain_themselves() {
    assert_eq!(
        Removal::VariantSuffix {
            base: index("PASB518")
        }
        .to_string(),
        "variant of PASB518"
    );
    assert_eq!(
        Removal::Manual {
            reason: ExcludeReason::BadSilhouette,
            base: None
        }
        .to_string(),
        "excluded by curation: bad silhouette"
    );
    assert_eq!(
        Removal::SharedHull {
            base: index("PJSB013")
        }
        .to_string(),
        "same hull model as PJSB013"
    );
}

#[test]
fn manual_exclusions_are_never_chosen_as_a_base() {
    let ships = catalog(
        1,
        vec![
            ship("PASD019", "Clemson", "upgradeable", 4, "db18"),
            ship("PASD704", "DD 214", "specialUnsellable", 4, "db18"),
        ],
    );
    let curated = curate(
        &ships,
        &config("[[exclude]]\nindex = \"PASD019\"\nreason = \"carbon_copy\""),
    );
    assert!(curated.pool.contains(&index("PASD704")));
    assert_eq!(
        curated.removed[&index("PASD019")],
        Removal::Manual {
            reason: ExcludeReason::CarbonCopy,
            base: None
        }
    );
}

#[test]
fn a_silhouette_shared_only_by_reskins_has_no_base() {
    let ships = catalog(
        1,
        vec![
            ship("PJSC705", "ARP Myoko", "special", 7, "a7a7"),
            ship("PJSC709", "ARP Haguro", "special", 7, "a7a7"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(
        curated.removed[&index("PJSC705")],
        Removal::CollaborationPrefix
    );
    assert_eq!(
        curated.removed[&index("PJSC709")],
        Removal::CollaborationPrefix
    );
    assert!(curated.variants_of(&index("PJSC705")).is_empty());
}

#[test]
fn a_ship_sharing_a_bad_silhouette_leaves_the_pool() {
    let ships = catalog(
        1,
        vec![
            ship("PASD019", "Clemson", "upgradeable", 4, "db18"),
            ship("PASD704", "DD 214", "specialUnsellable", 4, "db18"),
            ship("PBSC507", "Belfast", "special", 7, "c866"),
        ],
    );
    let curated = curate(
        &ships,
        &config("[[exclude]]\nindex = \"PASD019\"\nreason = \"bad_silhouette\""),
    );
    assert_eq!(
        curated.removed[&index("PASD704")],
        Removal::SharesBadSilhouette {
            with: index("PASD019")
        }
    );
    assert_eq!(
        curated.pool.iter().map(|i| i.as_str()).collect::<Vec<_>>(),
        ["PBSC507"]
    );
}

#[test]
fn a_copy_of_a_variant_points_at_the_pool_base() {
    let ships = catalog(
        1,
        vec![
            ship("PASB518", "Massachusetts", "special", 8, "23ba"),
            ship("PASB598", "Massachusetts B", "special", 8, "ebd0"),
            ship("PASB798", "Massachusetts Golden", "special", 8, "ebd0"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(
        curated.removed[&index("PASB598")],
        Removal::VariantSuffix {
            base: index("PASB518")
        }
    );
    assert_eq!(
        curated.removed[&index("PASB798")],
        Removal::CopyOf {
            via: index("PASB598"),
            base: index("PASB518")
        }
    );
    assert_eq!(
        curated.variants_of(&index("PASB518")),
        vec![&index("PASB598"), &index("PASB798")]
    );
}

#[test]
fn an_excluded_ship_is_never_a_variant_base() {
    let ships = catalog(
        1,
        vec![
            ship("PASB518", "Massachusetts", "special", 8, "23ba"),
            ship("PASB598", "Massachusetts B", "special", 8, "ebd0"),
        ],
    );
    let curated = curate(
        &ships,
        &config("[[exclude]]\nindex = \"PASB518\"\nreason = \"carbon_copy\""),
    );
    assert!(curated.pool.contains(&index("PASB598")));
}

#[test]
fn copies_explain_their_chain() {
    assert_eq!(
        Removal::CopyOf {
            via: index("PASB598"),
            base: index("PASB518")
        }
        .to_string(),
        "copy of PASB598, which is a copy of PASB518"
    );
    assert_eq!(
        Removal::SharesBadSilhouette {
            with: index("PASD019")
        }
        .to_string(),
        "same silhouette as PASD019, which is excluded for bad silhouette art"
    );
}

#[test]
fn ships_sharing_a_hull_model_count_as_copies_of_one_base() {
    let ships = catalog(
        1,
        vec![
            hulled(
                ship("PJSB013", "Amagi", "upgradeable", 8, "a013"),
                AMAGI_HULL,
            ),
            hulled(
                ship("PJSB878", "Ignis Purgatio", "special", 8, "a878"),
                AMAGI_HULL,
            ),
            hulled(
                ship("PJSB888", "Ragnarok", "special", 8, "a888"),
                AMAGI_HULL,
            ),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(
        curated.pool.iter().map(|i| i.as_str()).collect::<Vec<_>>(),
        ["PJSB013"]
    );
    for copy in ["PJSB878", "PJSB888"] {
        assert_eq!(
            curated.removed[&index(copy)],
            Removal::SharedHull {
                base: index("PJSB013")
            }
        );
    }
    assert_eq!(
        curated.variants_of(&index("PJSB013")),
        vec![&index("PJSB878"), &index("PJSB888")]
    );
}

#[test]
fn a_kept_ship_survives_a_shared_hull() {
    let hull = "content/gameplay/poland/ship/destroyer/WSD207_Grom/WSD207_Grom.model";
    let ships = catalog(
        1,
        vec![
            hulled(ship("PWSD207", "Grom", "upgradeable", 7, "d207"), hull),
            hulled(ship("PWSD907", "Scarlet Storm", "special", 7, "d907"), hull),
        ],
    );
    let config = config("[[keep]]\nindex = \"PWSD907\"");
    let curated = curate(&ships, &config);
    assert!(curated.removed.is_empty());
    assert!(curated.pool.contains(&index("PWSD207")));
    assert!(curated.pool.contains(&index("PWSD907")));
    assert!(
        !validate(&ships, &config, &curated)
            .iter()
            .any(|problem| matches!(problem, Problem::KeepHasNoEffect { .. }))
    );
}

#[test]
fn a_premium_ship_is_preferred_as_base_over_a_later_special_copy() {
    let hull = "content/gameplay/italy/ship/battleship/ISB509_Marco_Polo/ISB509_Marco_Polo.model";
    let ships = catalog(
        1,
        vec![
            hulled(ship("PISB509", "Marco Polo", "premium", 9, "b509"), hull),
            hulled(ship("PISB709", "Pilus Prior", "special", 9, "b709"), hull),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert!(curated.pool.contains(&index("PISB509")));
    assert_eq!(
        curated.removed[&index("PISB709")],
        Removal::SharedHull {
            base: index("PISB509")
        }
    );
}

#[test]
fn ships_without_a_hull_model_are_never_grouped() {
    let ships = catalog(
        1,
        vec![
            ship("PJSB013", "Amagi", "upgradeable", 8, "a013"),
            ship("PJSB878", "Ignis Purgatio", "special", 8, "a878"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert!(curated.removed.is_empty());
    assert_eq!(curated.pool.len(), 2);
}

#[test]
fn a_manually_excluded_ship_is_never_a_hull_base() {
    let ships = catalog(
        1,
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
    let curated = curate(
        &ships,
        &config("[[exclude]]\nindex = \"PJSB013\"\nreason = \"carbon_copy\""),
    );
    assert_eq!(
        curated.pool.iter().map(|i| i.as_str()).collect::<Vec<_>>(),
        ["PJSB878"]
    );
    assert_eq!(
        curated.removed[&index("PJSB013")],
        Removal::Manual {
            reason: ExcludeReason::CarbonCopy,
            base: None
        }
    );
}

#[test]
fn a_manual_exclusion_replaces_a_shared_hull() {
    let ships = catalog(
        1,
        vec![
            hulled(
                ship("PJSB013", "Amagi", "upgradeable", 8, "a013"),
                AMAGI_HULL,
            ),
            hulled(
                ship("PJSB888", "Ragnarok", "special", 8, "a888"),
                AMAGI_HULL,
            ),
        ],
    );
    let curated = curate(
        &ships,
        &config("[[exclude]]\nindex = \"PJSB888\"\nreason = \"carbon_copy\"\nbase = \"PJSB013\""),
    );
    assert_eq!(
        curated.removed[&index("PJSB888")],
        Removal::Manual {
            reason: ExcludeReason::CarbonCopy,
            base: Some(index("PJSB013"))
        }
    );
}

#[test]
fn a_copy_of_a_shared_hull_copy_points_at_the_hull_base() {
    let ships = catalog(
        1,
        vec![
            hulled(
                ship("PJSB013", "Amagi", "upgradeable", 8, "a013"),
                AMAGI_HULL,
            ),
            hulled(
                ship("PJSB878", "Ignis Purgatio", "special", 8, "a878"),
                AMAGI_HULL,
            ),
            ship("PJSB898", "Ignis Purgatio Golden", "special", 8, "a898"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(
        curated.removed[&index("PJSB898")],
        Removal::CopyOf {
            via: index("PJSB878"),
            base: index("PJSB013")
        }
    );
}

#[test]
fn a_premium_ship_ranks_with_special_ships_as_a_silhouette_base() {
    let ships = catalog(
        1,
        vec![
            ship("PZSB529", "Sun Yat-Sen", "premium", 9, "c529"),
            ship("PZSB909", "Xuan Ming", "special", 9, "c529"),
        ],
    );
    let curated = curate(&ships, &config(""));
    assert_eq!(
        curated.removed[&index("PZSB909")],
        Removal::IdenticalSilhouette {
            base: index("PZSB529")
        }
    );
}

use std::path::Path;

use barnacle_catalog::Provenance;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::curation::CurationConfig;
use barnacle_catalog::curation::Removal;
use barnacle_catalog::curation::curate;
use barnacle_data::extract::BuildInputs;
use barnacle_data::extract::build_catalog;
use wows_data_mgr::Dump;

fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

#[test]
fn builds_and_curates_a_real_catalog() {
    let Some(dir) = std::env::var_os("BARNACLE_TEST_BUILD_DIR") else {
        eprintln!("skipping: set BARNACLE_TEST_BUILD_DIR to a downloaded build directory");
        return;
    };
    let dump = Dump::open(Path::new(&dir));
    let english_mo = dump
        .derived_path("translations/en/LC_MESSAGES/global.mo")
        .unwrap();
    let output = tempfile::tempdir().unwrap();
    let vfs = dump.vfs();
    let catalog = build_catalog(BuildInputs {
        vfs: &vfs,
        english_mo: &english_mo,
        provenance: Provenance {
            game_version: "test".to_owned(),
            build: 0,
            data_repo_commit: "test".to_owned(),
            wowsunpack: "test".to_owned(),
            wows_data_mgr: "test".to_owned(),
        },
        output_dir: output.path(),
    })
    .unwrap();

    assert!(
        catalog.ships.len() > 1000,
        "only {} ships",
        catalog.ships.len()
    );
    let colorado = catalog.get(&index("PASB008")).unwrap();
    assert_eq!(colorado.tier.get(), 7);
    assert_eq!(colorado.class, ShipClass::Battleship);
    assert_eq!(colorado.nation.as_str(), "USA");
    assert!(!colorado.is_paper);
    assert_eq!(
        colorado.name.as_ref().map(|name| name.short.as_str()),
        Some("Colorado")
    );
    assert!(colorado.silhouette.is_some());
    assert!(output.path().join("silhouettes/PASB008.png").is_file());
    assert!(catalog.get(&index("PASB110")).unwrap().is_paper);

    let config = CurationConfig::from_toml(
        r#"groups = ["start", "special", "specialUnsellable", "ultimate", "upgradeable", "upgradeableExclusive", "upgradeableUltimate", "superShip"]"#,
    )
    .unwrap();
    let curated = curate(&catalog, &config);
    assert_eq!(
        curated.removed[&index("PGSC899")],
        Removal::IdenticalSilhouette {
            base: index("PGSC519")
        }
    );
    assert_eq!(
        curated.removed[&index("PASB598")],
        Removal::VariantSuffix {
            base: index("PASB518")
        }
    );
    assert_eq!(
        curated.removed[&index("PJSC708")],
        Removal::CollaborationPrefix
    );
    for kept in [
        "PASB008", "PASD709", "PBSC101", "PGSB105", "PGSB503", "PBSC507", "PBSC528",
    ] {
        assert!(
            curated.pool.contains(&index(kept)),
            "{kept} should be in the pool"
        );
    }
    assert!(
        catalog
            .ships
            .iter()
            .filter(|ship| ship.group.as_str().starts_with("demo"))
            .all(|ship| !curated.pool.contains(&ship.index)),
        "a test ship reached the pool"
    );
}

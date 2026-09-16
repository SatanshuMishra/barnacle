use barnacle_catalog::Catalog;
use barnacle_catalog::Provenance;
use barnacle_data::store::DataDir;
use barnacle_data::store::StoreError;

fn empty_catalog(build: u32) -> Catalog {
    Catalog {
        provenance: Provenance {
            game_version: "15.8.0".to_owned(),
            build,
            data_repo_commit: "0000000".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        ships: Vec::new(),
    }
}

fn data_dir_with(names: &[&str]) -> (tempfile::TempDir, DataDir) {
    let root = tempfile::tempdir().unwrap();
    let data = DataDir::new(root.path());
    for name in names {
        std::fs::create_dir_all(data.catalog_dir(name)).unwrap();
        data.save(name, &empty_catalog(1)).unwrap();
    }
    (root, data)
}

#[test]
fn current_is_absent_until_set() {
    let (_root, data) = data_dir_with(&["15.8.0_13187581"]);
    assert_eq!(data.current().unwrap(), None);
    data.set_current("15.8.0_13187581").unwrap();
    assert_eq!(data.current().unwrap().as_deref(), Some("15.8.0_13187581"));
}

#[test]
fn built_catalogs_are_ordered_by_build_number_not_by_name() {
    let (_root, data) = data_dir_with(&["15.8.0_13187581", "9.9.1_2979658", "15.7.0_13015811"]);
    std::fs::create_dir_all(data.catalog_dir("15.9.0_13300000")).unwrap();
    assert_eq!(
        data.built().unwrap(),
        ["9.9.1_2979658", "15.7.0_13015811", "15.8.0_13187581"]
    );
    assert_eq!(data.newest().unwrap(), "15.8.0_13187581");
}

#[test]
fn an_empty_data_directory_has_no_catalogs() {
    let (_root, data) = data_dir_with(&[]);
    assert_eq!(data.built().unwrap(), Vec::<String>::new());
    assert!(matches!(data.newest(), Err(StoreError::NoCatalogs)));
}

#[test]
fn saved_catalogs_load_back() {
    let (_root, data) = data_dir_with(&["15.8.0_13187581"]);
    assert_eq!(data.load("15.8.0_13187581").unwrap(), empty_catalog(1));
    assert!(matches!(
        data.load("15.7.0_13015811"),
        Err(StoreError::Io { .. })
    ));
}

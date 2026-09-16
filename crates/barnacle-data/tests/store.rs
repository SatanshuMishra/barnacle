use barnacle_catalog::Catalog;
use barnacle_catalog::Provenance;
use barnacle_data::store::DataDir;
use barnacle_data::store::StoreError;
use barnacle_data::store::check_build_dir;
use barnacle_data::store::pinned_base_url;
use wows_data_mgr::builds::BuildEntry;

const COMMIT: &str = "3f7a1c2b9d8e4f6a0b1c2d3e4f5a6b7c8d9e0f1a";

fn empty_catalog(build: u32) -> Catalog {
    Catalog {
        provenance: Provenance {
            game_version: "15.8.0".to_owned(),
            build,
            data_repo_commit: COMMIT.to_owned(),
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

fn entry(version: &str, build: u32, dir: &str) -> BuildEntry {
    BuildEntry {
        version: version.to_owned(),
        build,
        dir: dir.to_owned(),
        dumped_at: "2026-09-10T08:56:09-07:00".to_owned(),
    }
}

#[test]
fn current_is_absent_until_set() {
    let (_root, data) = data_dir_with(&["15.8.0_13187581_r1"]);
    assert_eq!(data.current().unwrap(), None);
    data.set_current("15.8.0_13187581_r1").unwrap();
    assert_eq!(
        data.current().unwrap().as_deref(),
        Some("15.8.0_13187581_r1")
    );
}

#[test]
fn built_catalogs_are_ordered_by_build_then_revision() {
    let (_root, data) = data_dir_with(&[
        "15.8.0_13187581_r2",
        "9.9.1_2979658_r1",
        "15.8.0_13187581_r1",
        "15.7.0_13015811_r1",
    ]);
    std::fs::create_dir_all(data.catalog_dir("15.9.0_13300000_r1")).unwrap();
    std::fs::create_dir_all(data.catalog_dir("15.8.0_13187581")).unwrap();
    assert_eq!(
        data.built().unwrap(),
        [
            "9.9.1_2979658_r1",
            "15.7.0_13015811_r1",
            "15.8.0_13187581_r1",
            "15.8.0_13187581_r2"
        ]
    );
    assert_eq!(data.newest().unwrap(), "15.8.0_13187581_r2");
}

#[test]
fn every_build_reserves_a_new_catalog_directory() {
    let (_root, data) = data_dir_with(&["15.8.0_13187581_r1"]);
    data.set_current("15.8.0_13187581_r1").unwrap();
    let before =
        std::fs::read_to_string(data.catalog_dir("15.8.0_13187581_r1").join("catalog.json"))
            .unwrap();

    let (second, second_dir) = data.reserve_catalog_dir("15.8.0_13187581").unwrap();
    let (third, third_dir) = data.reserve_catalog_dir("15.8.0_13187581").unwrap();
    let (other, _) = data.reserve_catalog_dir("15.9.0_13300000").unwrap();

    assert_eq!(second, "15.8.0_13187581_r2");
    assert_eq!(third, "15.8.0_13187581_r3");
    assert_eq!(other, "15.9.0_13300000_r1");
    assert!(second_dir.is_dir() && third_dir.is_dir());
    assert_eq!(
        std::fs::read_to_string(data.catalog_dir("15.8.0_13187581_r1").join("catalog.json"))
            .unwrap(),
        before
    );
}

#[test]
fn an_empty_data_directory_has_no_catalogs() {
    let (_root, data) = data_dir_with(&[]);
    assert_eq!(data.built().unwrap(), Vec::<String>::new());
    assert!(matches!(data.newest(), Err(StoreError::NoCatalogs)));
}

#[test]
fn saved_catalogs_load_back() {
    let (_root, data) = data_dir_with(&["15.8.0_13187581_r1"]);
    assert_eq!(data.load("15.8.0_13187581_r1").unwrap(), empty_catalog(1));
    assert!(matches!(
        data.load("15.7.0_13015811_r1"),
        Err(StoreError::Io { .. })
    ));
}

#[test]
fn downloads_are_pinned_to_one_commit() {
    assert_eq!(
        pinned_base_url(wows_data_mgr::download_repo::DEFAULT_REPO_BASE_URL, COMMIT).unwrap(),
        format!("https://raw.githubusercontent.com/landaire/wows-replay-data/{COMMIT}")
    );
    assert!(matches!(
        pinned_base_url(
            wows_data_mgr::download_repo::DEFAULT_REPO_BASE_URL,
            "main/../x"
        ),
        Err(StoreError::UnexpectedCommit { .. })
    ));
    assert!(matches!(
        pinned_base_url("https://example.com/data", COMMIT),
        Err(StoreError::UnexpectedRepoUrl { .. })
    ));
}

#[test]
fn build_directories_from_the_remote_index_are_checked() {
    assert!(check_build_dir(&entry("15.8.0", 13187581, "15.8.0_13187581")).is_ok());
    for dir in [
        "../15.8.0_13187581",
        "/tmp/15.8.0_13187581",
        "15.8.0_13187582",
        "",
    ] {
        assert!(
            matches!(
                check_build_dir(&entry("15.8.0", 13187581, dir)),
                Err(StoreError::UnsafeBuildDir { .. })
            ),
            "{dir} should be rejected"
        );
    }
    assert!(matches!(
        check_build_dir(&entry("../x", 1, "../x_1")),
        Err(StoreError::UnsafeBuildDir { .. })
    ));
}

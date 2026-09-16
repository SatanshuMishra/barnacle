use std::path::Path;
use std::path::PathBuf;

use barnacle_catalog::Catalog;
use barnacle_catalog::Provenance;
use barnacle_data::extract::ExtractError;
use barnacle_data::store::DataDir;
use barnacle_data::store::Downloaded;
use barnacle_data::store::StoreError;
use barnacle_data::store::build;
use barnacle_data::store::check_build_dir;
use barnacle_data::store::pinned_base_url;
use image::ImageFormat;
use image::Rgba;
use image::RgbaImage;
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

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn data_dir_with(names: &[&str]) -> (tempfile::TempDir, DataDir) {
    let root = tempfile::tempdir().unwrap();
    let data = DataDir::new(root.path());
    for name in names {
        write(
            &data.catalog_dir(name).join("catalog.json"),
            empty_catalog(1).to_json().unwrap().as_bytes(),
        );
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

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn silhouette_png() -> Vec<u8> {
    let image = RgbaImage::from_pixel(4, 2, Rgba([38, 29, 26, 255]));
    let mut cursor = std::io::Cursor::new(Vec::new());
    image.write_to(&mut cursor, ImageFormat::Png).unwrap();
    cursor.into_inner()
}

fn downloaded_plain_build(data: &DataDir, game_params: &str) -> Downloaded {
    let dir = data.store().join("15.8.0_13187581");
    write(
        &dir.join("vfs/content/GameParams.data"),
        &std::fs::read(fixture(game_params)).unwrap(),
    );
    write(
        &dir.join("vfs/gui/ships_silhouettes/PASB008.png"),
        &silhouette_png(),
    );
    write(
        &dir.join("translations/en/LC_MESSAGES/global.mo"),
        &std::fs::read(fixture("mini_en.mo")).unwrap(),
    );
    Downloaded {
        entry: entry("15.8.0", 13187581, "15.8.0_13187581"),
        data_repo_commit: COMMIT.to_owned(),
    }
}

fn catalog_entries(data: &DataDir) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(data.catalogs())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
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
        "15.9.0_13187581_r1",
    ]);
    std::fs::create_dir_all(data.catalog_dir("15.9.0_13300000_r1")).unwrap();
    assert_eq!(
        data.built().unwrap(),
        [
            "9.9.1_2979658_r1",
            "15.7.0_13015811_r1",
            "15.8.0_13187581_r1",
            "15.9.0_13187581_r1",
            "15.8.0_13187581_r2"
        ]
    );
    assert_eq!(data.newest().unwrap(), "15.8.0_13187581_r2");
}

#[test]
fn non_canonical_catalog_names_are_ignored() {
    let (_root, data) = data_dir_with(&[
        "15.8.0_13187581",
        "15.8.0_13187581_r01",
        "15.8.0_13187581_r+1",
        "15.8.0_13187581_r0",
        "15.8.0_013187581_r1",
        "15.8.0_13187581_r1",
    ]);
    assert_eq!(data.built().unwrap(), ["15.8.0_13187581_r1"]);
}

#[test]
fn a_revision_counter_that_would_overflow_is_an_error() {
    let (_root, data) = data_dir_with(&["15.8.0_13187581_r4294967295"]);
    let error = build(&data, &downloaded_plain_build(&data, "catalog_ok.data")).err();
    assert!(matches!(error, Some(StoreError::RevisionOverflow { .. })));
}

#[test]
fn rebuilding_never_touches_an_existing_catalog() {
    let (_root, data) = data_dir_with(&[]);
    let downloaded = downloaded_plain_build(&data, "catalog_ok.data");

    let first = build(&data, &downloaded).unwrap();
    assert_eq!(first.name, "15.8.0_13187581_r1");
    assert_eq!(first.catalog.provenance.data_repo_commit, COMMIT);
    data.set_current(&first.name).unwrap();
    let before = std::fs::read(data.catalog_dir(&first.name).join("catalog.json")).unwrap();

    let second = build(&data, &downloaded).unwrap();
    assert_eq!(second.name, "15.8.0_13187581_r2");
    assert_eq!(
        std::fs::read(data.catalog_dir(&first.name).join("catalog.json")).unwrap(),
        before
    );
    assert!(
        data.catalog_dir(&second.name)
            .join("silhouettes/PASB008.png")
            .is_file()
    );
    assert_eq!(
        catalog_entries(&data),
        ["15.8.0_13187581_r1", "15.8.0_13187581_r2", "current"]
    );
}

#[test]
fn a_failed_build_leaves_nothing_behind() {
    let (_root, data) = data_dir_with(&[]);
    let error = build(
        &data,
        &downloaded_plain_build(&data, "catalog_unparsable.data"),
    )
    .err();
    assert!(matches!(
        error,
        Some(StoreError::Extract(ExtractError::UnparsedShips { .. }))
    ));
    assert_eq!(catalog_entries(&data), Vec::<String>::new());
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

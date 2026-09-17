mod common;

use barnacle_catalog::store::CatalogDirError;
use barnacle_catalog::store::CatalogRoot;
use common::catalog;
use common::index;

fn root() -> (tempfile::TempDir, CatalogRoot) {
    let dir = tempfile::tempdir().unwrap();
    let root = CatalogRoot::new(dir.path());
    (dir, root)
}

#[test]
fn no_current_file_means_no_current_catalog() {
    let (_dir, root) = root();
    assert_eq!(root.current().unwrap(), None);
}

#[test]
fn the_current_file_names_the_catalog_without_surrounding_whitespace() {
    let (_dir, root) = root();
    std::fs::write(root.path().join("current"), "15.8.0_13187581_r4\n").unwrap();
    assert_eq!(
        root.current().unwrap().as_deref(),
        Some("15.8.0_13187581_r4")
    );
}

#[test]
fn a_saved_catalog_loads_back() {
    let (_dir, root) = root();
    let saved = catalog(13187581, Vec::new());
    std::fs::create_dir_all(root.dir("15.8.0_13187581_r4")).unwrap();
    std::fs::write(
        root.dir("15.8.0_13187581_r4").join("catalog.json"),
        saved.to_json().unwrap(),
    )
    .unwrap();
    assert_eq!(root.load("15.8.0_13187581_r4").unwrap(), saved);
}

#[test]
fn a_missing_catalog_is_an_io_error_and_a_broken_one_a_json_error() {
    let (_dir, root) = root();
    assert!(matches!(
        root.load("15.8.0_13187581_r4"),
        Err(CatalogDirError::Io { .. })
    ));
    std::fs::create_dir_all(root.dir("15.8.0_13187581_r5")).unwrap();
    std::fs::write(root.dir("15.8.0_13187581_r5").join("catalog.json"), "{").unwrap();
    assert!(matches!(
        root.load("15.8.0_13187581_r5"),
        Err(CatalogDirError::Json { .. })
    ));
}

#[test]
fn a_silhouette_lives_in_the_catalogs_silhouettes_folder() {
    let (_dir, root) = root();
    assert_eq!(
        root.silhouette("15.8.0_13187581_r4", &index("PASB008")),
        root.path()
            .join("15.8.0_13187581_r4")
            .join("silhouettes")
            .join("PASB008.png")
    );
}

use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use barnacle_catalog::Catalog;
use barnacle_catalog::Provenance;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipIndex;
use barnacle_data::extract::BuildInputs;
use barnacle_data::extract::ExtractError;
use barnacle_data::extract::build_catalog;
use barnacle_data::extract::silhouette::sha256_hex;
use image::ImageFormat;
use image::Rgba;
use image::RgbaImage;
use wowsunpack::vfs::MemoryFS;
use wowsunpack::vfs::VfsPath;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn silhouette_png() -> Vec<u8> {
    let image = RgbaImage::from_fn(4, 2, |x, _| {
        if x == 0 {
            Rgba([38, 29, 26, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    let mut cursor = std::io::Cursor::new(Vec::new());
    image.write_to(&mut cursor, ImageFormat::Png).unwrap();
    cursor.into_inner()
}

fn write(root: &VfsPath, path: &str, bytes: &[u8]) {
    let file = root.join(path).unwrap();
    file.parent().create_dir_all().unwrap();
    file.create_file().unwrap().write_all(bytes).unwrap();
}

fn build(
    game_params: &str,
    silhouettes: &[&str],
    english: &str,
) -> (tempfile::TempDir, Result<Catalog, ExtractError>) {
    let vfs = VfsPath::new(MemoryFS::new());
    write(
        &vfs,
        "content/GameParams.data",
        &std::fs::read(fixture(game_params)).unwrap(),
    );
    for index in silhouettes {
        write(
            &vfs,
            &format!("gui/ships_silhouettes/{index}.png"),
            &silhouette_png(),
        );
    }
    let output = tempfile::tempdir().unwrap();
    let result = build_catalog(BuildInputs {
        vfs: &vfs,
        english_mo: &fixture(english),
        provenance: Provenance {
            game_version: "15.8.0".to_owned(),
            build: 13187581,
            data_repo_commit: "0000000".to_owned(),
            wowsunpack: "0.45.0".to_owned(),
            wows_data_mgr: "0.21.0".to_owned(),
        },
        output_dir: output.path(),
    });
    (output, result)
}

fn index(value: &str) -> ShipIndex {
    ShipIndex::parse(value).unwrap()
}

#[test]
fn builds_ships_with_names_paper_flags_and_silhouettes() {
    let (output, result) = build("catalog_ok.data", &["PASB008"], "mini_en.mo");
    let catalog = result.unwrap();
    assert_eq!(
        catalog
            .ships
            .iter()
            .map(|ship| ship.index.as_str())
            .collect::<Vec<_>>(),
        ["PASB008", "PASB110"]
    );

    let colorado = catalog.get(&index("PASB008")).unwrap();
    assert_eq!(colorado.tier.get(), 7);
    assert_eq!(colorado.class, ShipClass::Battleship);
    assert_eq!(colorado.group.as_str(), "upgradeable");
    assert_eq!(colorado.nation.as_str(), "USA");
    assert!(!colorado.is_paper);
    assert_eq!(
        colorado.name.as_ref().map(|name| name.display()),
        Some("Colorado")
    );
    assert_eq!(
        colorado
            .silhouette
            .as_ref()
            .map(|silhouette| silhouette.sha256.clone()),
        Some(sha256_hex(&silhouette_png()))
    );
    assert!(output.path().join("silhouettes/PASB008.png").is_file());

    let vermont = catalog.get(&index("PASB110")).unwrap();
    assert!(vermont.is_paper);
    assert_eq!(vermont.name, None);
    assert_eq!(vermont.silhouette, None);
    assert!(!output.path().join("silhouettes/PASB110.png").exists());
}

#[test]
fn ships_the_parser_cannot_read_fail_the_build() {
    let (_output, result) = build("mini_gameparams.data", &["PASB008"], "mini_en.mo");
    assert!(
        matches!(result, Err(ExtractError::UnparsedShips { ref indices }) if indices == &["PASB008".to_owned(), "PASB110".to_owned()])
    );
    let (_output, result) = build("catalog_unparsable.data", &["PASB008"], "mini_en.mo");
    assert!(
        matches!(result, Err(ExtractError::UnparsedShips { ref indices }) if indices == &["PJSB018".to_owned()])
    );
}

#[test]
fn a_build_without_ships_fails() {
    let (_output, result) = build("catalog_no_ships.data", &[], "mini_en.mo");
    assert!(matches!(result, Err(ExtractError::NoShips)));
}

#[test]
fn a_build_without_any_silhouette_fails() {
    let (_output, result) = build("catalog_ok.data", &[], "mini_en.mo");
    assert!(matches!(result, Err(ExtractError::NoSilhouettes)));
}

#[test]
fn a_build_without_any_english_name_fails() {
    let (_output, result) = build("catalog_ok.data", &["PASB008"], "mini_en_empty.mo");
    assert!(matches!(result, Err(ExtractError::NoEnglishNames)));
}

#[test]
fn a_ship_index_listed_twice_fails() {
    let (_output, result) = build("catalog_duplicate.data", &["PASB008"], "mini_en.mo");
    assert!(matches!(
        result,
        Err(ExtractError::DuplicateIndex {
            listed: 2,
            unique: 1
        })
    ));
}

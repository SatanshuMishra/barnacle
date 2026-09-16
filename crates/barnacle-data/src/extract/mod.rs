pub mod paper;
pub mod params;
pub mod silhouette;
pub mod translations;

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;

use barnacle_catalog::Catalog;
use barnacle_catalog::Provenance;
use barnacle_catalog::Ship;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::Silhouette;
use wowsunpack::vfs::VfsError;
use wowsunpack::vfs::VfsPath;

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("game data could not be read")]
    Vfs(#[from] VfsError),
    #[error("a file could not be read or written")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Paper(#[from] paper::PaperError),
    #[error(transparent)]
    Params(#[from] params::ParamsError),
    #[error(transparent)]
    Names(#[from] translations::NamesError),
    #[error("a silhouette image could not be processed")]
    Image(#[from] image::ImageError),
    #[error("ship {index} has no paper-ship flag")]
    NoPaperFlag { index: ShipIndex },
    #[error("GameParams lists {listed} ships but only {unique} distinct indices")]
    DuplicateIndex { listed: usize, unique: usize },
    #[error("wowsunpack could not parse these ships: {indices:?}")]
    UnparsedShips { indices: Vec<String> },
    #[error("GameParams contains no ships")]
    NoShips,
    #[error("no ship has a silhouette; the game data is incomplete")]
    NoSilhouettes,
    #[error("no ship has an English name; the translation catalog does not match")]
    NoEnglishNames,
}

pub struct BuildInputs<'a> {
    pub vfs: &'a VfsPath,
    pub english_mo: &'a Path,
    pub provenance: Provenance,
    pub output_dir: &'a Path,
}

fn read(vfs: &VfsPath, path: &str) -> Result<Vec<u8>, ExtractError> {
    let mut bytes = Vec::new();
    vfs.join(path)?.open_file()?.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_if_present(vfs: &VfsPath, path: &str) -> Result<Option<Vec<u8>>, ExtractError> {
    if vfs.join(path)?.exists()? {
        read(vfs, path).map(Some)
    } else {
        Ok(None)
    }
}

pub fn build_catalog(inputs: BuildInputs<'_>) -> Result<Catalog, ExtractError> {
    let game_params = read(inputs.vfs, "content/GameParams.data")?;
    let paper = paper::paper_flags(game_params.clone())?;
    let typed = params::typed_ships(game_params)?;
    let parsed: BTreeSet<&str> = typed.iter().map(|ship| ship.index.as_str()).collect();
    let unparsed: Vec<String> = paper
        .keys()
        .filter(|index| !parsed.contains(index.as_str()))
        .cloned()
        .collect();
    if !unparsed.is_empty() {
        return Err(ExtractError::UnparsedShips { indices: unparsed });
    }
    if typed.is_empty() {
        return Err(ExtractError::NoShips);
    }
    let names = translations::EnglishNames::load(inputs.english_mo)?;
    let silhouettes = inputs.output_dir.join("silhouettes");
    std::fs::create_dir_all(&silhouettes)?;

    let ships = typed
        .into_iter()
        .map(|typed| {
            let is_paper =
                *paper
                    .get(typed.index.as_str())
                    .ok_or_else(|| ExtractError::NoPaperFlag {
                        index: typed.index.clone(),
                    })?;
            let silhouette = read_if_present(
                inputs.vfs,
                &format!("gui/ships_silhouettes/{}.png", typed.index),
            )?
            .map(|png| -> Result<Silhouette, ExtractError> {
                let composed = silhouette::composite(&png, silhouette::BACKGROUND)?;
                std::fs::write(silhouettes.join(format!("{}.png", typed.index)), composed)?;
                Ok(Silhouette {
                    sha256: silhouette::sha256_hex(&png),
                })
            })
            .transpose()?;
            Ok(Ship {
                name: names.ship_name(&typed.index),
                id: typed.id,
                index: typed.index,
                tier: typed.tier,
                group: typed.group,
                class: typed.class,
                nation: typed.nation,
                is_paper,
                silhouette,
            })
        })
        .collect::<Result<Vec<Ship>, ExtractError>>()?;

    let listed = ships.len();
    let by_index: BTreeMap<ShipIndex, Ship> = ships
        .into_iter()
        .map(|ship| (ship.index.clone(), ship))
        .collect();
    if by_index.len() != listed {
        return Err(ExtractError::DuplicateIndex {
            listed,
            unique: by_index.len(),
        });
    }
    if by_index.values().all(|ship| ship.silhouette.is_none()) {
        return Err(ExtractError::NoSilhouettes);
    }
    if by_index.values().all(|ship| ship.name.is_none()) {
        return Err(ExtractError::NoEnglishNames);
    }
    Ok(Catalog {
        provenance: inputs.provenance,
        ships: by_index.into_values().collect(),
    })
}

use barnacle_catalog::ModelError;
use barnacle_catalog::Nation;
use barnacle_catalog::ParamId;
use barnacle_catalog::ShipClass;
use barnacle_catalog::ShipGroup;
use barnacle_catalog::ShipIndex;
use barnacle_catalog::Tier;
use wowsunpack::error::GameDataError;
use wowsunpack::game_params::provider::GameMetadataProvider;
use wowsunpack::game_params::types::Species;
use wowsunpack::recognized::Recognized;

#[derive(Debug, thiserror::Error)]
pub enum ParamsError {
    #[error("GameParams could not be parsed")]
    Parse(#[from] GameDataError),
    #[error("a ship in GameParams has invalid data")]
    Model(#[from] ModelError),
}

pub struct TypedShip {
    pub id: ParamId,
    pub index: ShipIndex,
    pub tier: Tier,
    pub group: ShipGroup,
    pub class: ShipClass,
    pub nation: Nation,
}

pub fn typed_ships(game_params: Vec<u8>) -> Result<Vec<TypedShip>, ParamsError> {
    GameMetadataProvider::params_from_data(game_params)?
        .iter()
        .filter_map(|param| param.vehicle().map(|vehicle| (param, vehicle)))
        .map(|(param, vehicle)| {
            Ok(TypedShip {
                id: ParamId::new(param.id().raw()),
                index: ShipIndex::parse(param.index())?,
                tier: Tier::new(vehicle.level())?,
                group: ShipGroup::new(vehicle.group()),
                class: ship_class(param.species()),
                nation: Nation::new(param.nation()),
            })
        })
        .collect()
}

pub fn ship_class(species: Option<&Recognized<Species>>) -> ShipClass {
    match species {
        Some(Recognized::Known(Species::Destroyer)) => ShipClass::Destroyer,
        Some(Recognized::Known(Species::Cruiser)) => ShipClass::Cruiser,
        Some(Recognized::Known(Species::Battleship)) => ShipClass::Battleship,
        Some(Recognized::Known(Species::AirCarrier)) => ShipClass::AircraftCarrier,
        Some(Recognized::Known(Species::Submarine)) => ShipClass::Submarine,
        Some(Recognized::Known(other)) => ShipClass::Other(format!("{other:?}")),
        Some(Recognized::Unknown(raw)) => ShipClass::Other(raw.clone()),
        None => ShipClass::Unspecified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_the_five_playable_classes() {
        let cases = [
            (Species::Destroyer, ShipClass::Destroyer),
            (Species::Cruiser, ShipClass::Cruiser),
            (Species::Battleship, ShipClass::Battleship),
            (Species::AirCarrier, ShipClass::AircraftCarrier),
            (Species::Submarine, ShipClass::Submarine),
        ];
        for (species, class) in cases {
            assert_eq!(ship_class(Some(&Recognized::Known(species))), class);
        }
    }

    #[test]
    fn keeps_other_and_missing_species_visible() {
        assert_eq!(
            ship_class(Some(&Recognized::Known(Species::Auxiliary))),
            ShipClass::Other("Auxiliary".to_owned())
        );
        assert_eq!(
            ship_class(Some(&Recognized::Unknown("Hovercraft".to_owned()))),
            ShipClass::Other("Hovercraft".to_owned())
        );
        assert_eq!(ship_class(None), ShipClass::Unspecified);
    }
}

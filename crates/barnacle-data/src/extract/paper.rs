use std::collections::BTreeMap;

use pickled::HashableValue;
use pickled::Value;
use pickled::object::DictObject;
use pickled::value::Shared;
use wowsunpack::error::GameDataError;
use wowsunpack::game_params::convert::game_params_to_pickle;

#[derive(Debug, thiserror::Error)]
pub enum PaperError {
    #[error("GameParams could not be decoded")]
    Decode(#[from] GameDataError),
    #[error("GameParams root is not a dictionary, list or tuple")]
    UnexpectedRoot,
    #[error("GameParams has a wrapper entry that is not a dictionary")]
    UnexpectedWrapper,
    #[error("ships without a boolean isPaperShip: {indices:?}")]
    MissingFlag { indices: Vec<String> },
}

fn key(name: &str) -> HashableValue {
    HashableValue::String(name.to_owned().into())
}

fn as_dict(value: &Value) -> Option<Shared<pickled::Dict>> {
    match value {
        Value::Dict(dict) => Some(dict.clone()),
        Value::Object(object) => {
            let object = object.inner();
            let state = object
                .as_any()
                .downcast_ref::<DictObject>()?
                .state()
                .clone();
            Some(Shared::new(state))
        }
        _ => None,
    }
}

fn params_dict(root: &Value) -> Result<Shared<pickled::Dict>, PaperError> {
    if let Some(dict) = as_dict(root) {
        let wrapper = dict.inner().get(&key("")).cloned();
        return match wrapper {
            Some(wrapped) => as_dict(&wrapped).ok_or(PaperError::UnexpectedWrapper),
            None => Ok(dict),
        };
    }
    root.list_ref()
        .and_then(|list| list.inner().first().cloned())
        .or_else(|| {
            root.tuple_ref()
                .and_then(|tuple| tuple.inner().first().cloned())
        })
        .as_ref()
        .and_then(as_dict)
        .ok_or(PaperError::UnexpectedRoot)
}

fn string_field(entry: &pickled::Dict, name: &str) -> Option<String> {
    entry
        .get(&key(name))
        .and_then(Value::string_ref)
        .map(|text| text.inner().to_string())
}

fn is_ship(entry: &pickled::Dict) -> bool {
    entry
        .get(&key("typeinfo"))
        .and_then(as_dict)
        .and_then(|typeinfo| string_field(&typeinfo.inner(), "type"))
        .is_some_and(|kind| kind == "Ship")
}

pub fn paper_flags(game_params: Vec<u8>) -> Result<BTreeMap<String, bool>, PaperError> {
    let root = game_params_to_pickle(game_params)?;
    let params = params_dict(&root)?;
    let ships: Vec<(String, Option<bool>)> = params
        .inner()
        .values()
        .filter_map(as_dict)
        .filter_map(|entry| {
            let entry = entry.inner();
            if !is_ship(&entry) {
                return None;
            }
            let index = string_field(&entry, "index")?;
            let flag = entry
                .get(&key("isPaperShip"))
                .and_then(Value::bool_ref)
                .copied();
            Some((index, flag))
        })
        .collect();
    let missing: Vec<String> = ships
        .iter()
        .filter(|(_, flag)| flag.is_none())
        .map(|(index, _)| index.clone())
        .collect();
    if !missing.is_empty() {
        return Err(PaperError::MissingFlag { indices: missing });
    }
    Ok(ships
        .into_iter()
        .filter_map(|(index, flag)| flag.map(|flag| (index, flag)))
        .collect())
}

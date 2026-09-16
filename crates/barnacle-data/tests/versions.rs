fn locked_version(lock: &toml::Table, name: &str) -> Option<String> {
    lock.get("package")?
        .as_array()?
        .iter()
        .find(|package| package.get("name").and_then(toml::Value::as_str) == Some(name))?
        .get("version")?
        .as_str()
        .map(str::to_owned)
}

#[test]
fn recorded_toolkit_versions_match_the_lockfile() {
    let text =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock")).unwrap();
    let lock: toml::Table = toml::from_str(&text).unwrap();
    assert_eq!(
        locked_version(&lock, "wowsunpack").as_deref(),
        Some(barnacle_data::versions::WOWSUNPACK)
    );
    assert_eq!(
        locked_version(&lock, "wows-data-mgr").as_deref(),
        Some(barnacle_data::versions::WOWS_DATA_MGR)
    );
}

use std::path::PathBuf;

use barnacle_bot::config::CommandScope;
use barnacle_bot::config::Config;
use barnacle_bot::config::ConfigError;

#[test]
fn a_server_list_config_uses_the_default_paths() {
    let config = Config::from_toml(
        r#"
[commands]
scope = "guilds"
guilds = [123456789012345678]
"#,
    )
    .unwrap();
    assert_eq!(
        config,
        Config {
            data_dir: PathBuf::from("data"),
            curation: PathBuf::from("curation/ships.toml"),
            database: PathBuf::from("data/barnacle.sqlite3"),
            commands: CommandScope::Guilds {
                guilds: vec![123456789012345678],
            },
        }
    );
    assert_eq!(config.catalogs(), PathBuf::from("data/catalog"));
}

#[test]
fn a_global_config_can_set_every_path() {
    let config = Config::from_toml(
        r#"
data_dir = "/srv/barnacle/data"
curation = "/srv/barnacle/ships.toml"
database = "/srv/barnacle/barnacle.sqlite3"

[commands]
scope = "global"
"#,
    )
    .unwrap();
    assert_eq!(config.commands, CommandScope::Global);
    assert_eq!(
        config.catalogs(),
        PathBuf::from("/srv/barnacle/data/catalog")
    );
}

#[test]
fn a_server_list_must_name_real_servers() {
    assert!(matches!(
        Config::from_toml("[commands]\nscope = \"guilds\"\nguilds = []\n"),
        Err(ConfigError::NoGuilds)
    ));
    assert!(matches!(
        Config::from_toml("[commands]\nscope = \"guilds\"\nguilds = [0]\n"),
        Err(ConfigError::ZeroGuild)
    ));
}

#[test]
fn unknown_keys_and_a_missing_scope_are_refused() {
    assert!(matches!(
        Config::from_toml("[commands]\nscope = \"global\"\nguilds = [1]\n"),
        Err(ConfigError::GuildsWithGlobal)
    ));
    assert!(matches!(
        Config::from_toml("[commands]\nscope = \"guilds\"\n"),
        Err(ConfigError::NoGuilds)
    ));
    assert!(matches!(
        Config::from_toml("token = \"abc\"\n[commands]\nscope = \"global\"\n"),
        Err(ConfigError::Toml { .. })
    ));
    assert!(matches!(
        Config::from_toml("data_dir = \"data\"\n"),
        Err(ConfigError::Toml { .. })
    ));
}

#[test]
fn the_example_config_only_lacks_server_ids() {
    assert!(matches!(
        Config::from_toml(include_str!("../../../barnacle.example.toml")),
        Err(ConfigError::NoGuilds)
    ));
}

#[test]
fn a_config_error_never_repeats_what_the_file_contains() {
    for text in [
        "discord_token = \"MTIz.not-a-real-token\"\n\n[commands]\nscope = \"global\"\n",
        "discord_token = MTIz.not-a-real-token\n",
    ] {
        let error = Config::from_toml(text).unwrap_err();
        assert!(matches!(error, ConfigError::Toml { .. }));
        let chain: String = std::iter::successors(
            Some(&error as &(dyn std::error::Error + 'static)),
            |cause| cause.source(),
        )
        .map(|cause| cause.to_string())
        .collect();
        assert!(!chain.contains("not-a-real-token"));
        assert!(!format!("{error:?}").contains("not-a-real-token"));
    }
}

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
            rehearsal: Vec::new(),
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
    assert!(matches!(
        Config::from_toml(
            "[commands]\nscope = \"guilds\"\nguilds = [1]\n[rehearsals]\nguilds = [1]\n"
        ),
        Err(ConfigError::Toml { .. })
    ));
    assert!(matches!(
        Config::from_toml("[commands]\nscope = \"guilds\"\nguilds = [1]\n[rehearsal]\nguild = 1\n"),
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
        "[commands]\nscope = \"not-a-real-token\"\n",
        "[commands]\nscope = \"guilds\"\nguilds = [\"not-a-real-token\"]\n",
        "curation = 1234567890not-a-real-token\n[commands]\nscope = \"global\"\n",
        "[commands]\nscope = \"global\"\nnot-a-real-token = 1\n",
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

#[test]
fn a_config_error_names_where_the_problem_is() {
    let error = Config::from_toml("[commands]\nscope = \"everywhere\"\n").unwrap_err();
    assert!(matches!(error, ConfigError::Toml { line: 2, column: 9 }));
    assert_eq!(
        error.to_string(),
        "the config is not valid at line 2, column 9; compare it with barnacle.example.toml"
    );
}

#[test]
fn a_rehearsal_server_is_read_from_the_config() {
    let config = Config::from_toml(
        r#"
[commands]
scope = "guilds"
guilds = [111111111111111111, 222222222222222222]

[rehearsal]
guilds = [222222222222222222]
"#,
    )
    .unwrap();
    assert_eq!(
        config.commands,
        CommandScope::Guilds {
            guilds: vec![111111111111111111, 222222222222222222],
        }
    );
    assert_eq!(config.rehearsal, vec![222222222222222222]);
}

#[test]
fn a_rehearsal_server_outside_the_command_guilds_is_refused() {
    let error = Config::from_toml(
        r#"
[commands]
scope = "guilds"
guilds = [111111111111111111]

[rehearsal]
guilds = [111111111111111111, 333333333333333333]
"#,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ConfigError::RehearsalNotRegistered {
            guild: 333333333333333333
        }
    ));
    assert_eq!(
        error.to_string(),
        "rehearsal.guilds lists 333333333333333333, which is not in commands.guilds"
    );
}

#[test]
fn a_rehearsal_server_with_global_scope_is_refused() {
    let error = Config::from_toml(
        r#"
[commands]
scope = "global"

[rehearsal]
guilds = [222222222222222222]
"#,
    )
    .unwrap_err();
    assert!(matches!(error, ConfigError::RehearsalWithGlobal));
    assert_eq!(
        error.to_string(),
        "commands.scope is \"global\", so rehearsal.guilds must be left out"
    );
}

#[test]
fn a_zero_rehearsal_server_is_refused() {
    let error = Config::from_toml(
        r#"
[commands]
scope = "guilds"
guilds = [111111111111111111]

[rehearsal]
guilds = [0]
"#,
    )
    .unwrap_err();
    assert!(matches!(error, ConfigError::ZeroRehearsalGuild));
    assert_eq!(
        error.to_string(),
        "rehearsal.guilds contains 0, which is not a Discord server ID"
    );
}

#[test]
fn an_empty_rehearsal_list_is_no_rehearsal_server() {
    let with_section = Config::from_toml(
        r#"
[commands]
scope = "guilds"
guilds = [111111111111111111]

[rehearsal]
guilds = []
"#,
    )
    .unwrap();
    let without_section = Config::from_toml(
        r#"
[commands]
scope = "guilds"
guilds = [111111111111111111]
"#,
    )
    .unwrap();
    assert!(with_section.rehearsal.is_empty());
    assert_eq!(with_section, without_section);
}

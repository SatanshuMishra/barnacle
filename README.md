# Barnacle

A Discord bot for World of Warships players. Its first feature is a ship silhouette guessing game.

Ship data and silhouettes come from [wows-toolkit](https://github.com/landaire/wows-toolkit) and the game data it publishes.

## Updating game data

```bash
cargo run --release -p barnacle-data -- sync
cargo run --release -p barnacle-data -- diff
```

Review the new ships the diff lists, edit `curation/ships.toml`, set `reviewed_through` to the new build number, then:

```bash
cargo run --release -p barnacle-data -- use <version>_<build>
```

Downloaded game data and built catalogs live in `data/`, which is never committed.

## License

Apache-2.0. See `LICENSE`.

## Wargaming notice

Barnacle is an unofficial fan project. It is not affiliated with, endorsed by, or supported by Wargaming. World of Warships, its ship names and its ship silhouettes are trademarks or copyrighted works of Wargaming.

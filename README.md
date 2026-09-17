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
cargo run --release -p barnacle-data -- use <version>_<build>_r<n>
```

Every `sync` builds into a new `data/catalog/<version>_<build>_r<n>` directory, so it never changes the catalog the bot is using. `use` makes a validated catalog the one the bot loads on its next start.

Downloaded game data and built catalogs live in `data/`, which is never committed.

## Running the bot

Barnacle runs on your own machine and serves the catalog that `use` selected.

1. In the [Discord Developer Portal](https://discord.com/developers/applications), create an application, add a bot to it, and copy the bot token.
2. On the Bot page, turn on Message Content Intent under Privileged Gateway Intents. Barnacle reads chat messages to check answers.
3. In the application settings, turn off Public Bot, so only you can add Barnacle to servers.
4. Invite the bot. With Public Bot off, the portal offers no install link, so build one from the Application ID on the General Information page:

   ```text
   https://discord.com/oauth2/authorize?client_id=<Application ID>&scope=bot%20applications.commands&permissions=274878024704
   ```

   Open it while signed in as the application's owner, pick the server, and authorize. The permissions number grants View Channels, Send Messages, Send Messages in Threads, Embed Links, Attach Files and Read Message History. Barnacle replies to the winning message, which needs Read Message History, and rounds in threads need Send Messages in Threads. Keep Requires OAuth2 Code Grant off on the Bot page.
5. Copy `barnacle.example.toml` to `barnacle.toml` and list the IDs of the servers to register commands in. Once the bot should work in every server you add it to, change the scope to `"global"` and remove `guilds`.
6. Create the solves database once:

   ```bash
   sqlite3 data/barnacle.sqlite3 < migrations/0001_guess_solves.sql
   ```

7. Start the bot. Reading the token with `read` keeps it out of your shell history:

   ```bash
   read -rs DISCORD_TOKEN && export DISCORD_TOKEN && cargo run --release -p barnacle-bot
   ```

Before it connects, the bot checks the catalog, the curation file, the silhouettes and the database, and names anything that is missing.

### Player data

`data/barnacle.sqlite3` stores the server ID, the player's Discord user ID, the ship and the time of every won round. It stores no names and no messages.

- To delete a player's data, stop the bot and run `sqlite3 data/barnacle.sqlite3 "DELETE FROM guess_solves WHERE user_id = <user ID>;"`.
- To back the data up, copy the file while the bot is stopped.
- `migrations/0001_guess_solves.down.sql` removes the table and every stored solve with it. Back the file up before running it.

## License

Apache-2.0. See `LICENSE`.

## Wargaming notice

Barnacle is an unofficial fan project. It is not affiliated with, endorsed by, or supported by Wargaming. World of Warships, its ship names and its ship silhouettes are trademarks or copyrighted works of Wargaming.

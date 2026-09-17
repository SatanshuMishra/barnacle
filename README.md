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

7. Create the Clan Battle attendance tables once:

   ```bash
   sqlite3 data/barnacle.sqlite3 < migrations/0002_cb_attendance.sql
   ```

8. Add the Clan Battle season controls once. This rebuilds `cb_seasons`, so back the database up first:

   ```bash
   cp data/barnacle.sqlite3 data/barnacle.sqlite3.bak
   ```

   Then, with the bot stopped:

   ```bash
   sqlite3 data/barnacle.sqlite3 < migrations/0003_cb_season_controls.sql
   ```

   This file runs once: a second run aborts on its own guard before touching anything. Run it with the `sqlite3` command as shown, because that guard relies on the command stopping at the first error.

9. Start the bot. Reading the token with `read` keeps it out of your shell history:

   ```bash
   read -rs DISCORD_TOKEN && export DISCORD_TOKEN && cargo run --release -p barnacle-bot
   ```

Before it connects, the bot checks the catalog, the curation file, the silhouettes and the database, and names anything that is missing.

### Player data

`data/barnacle.sqlite3` stores the server ID, the player's Discord user ID, the ship and the time of every won round. It stores no names and no messages.

- To delete a player's data, stop the bot and run `sqlite3 data/barnacle.sqlite3 "DELETE FROM guess_solves WHERE user_id = <user ID>;"`.
- To back the data up, copy the file while the bot is stopped.
- `migrations/0001_guess_solves.down.sql` removes the table and every stored solve with it. Back the file up before running it.

For Clan Battle sign-ups, the same database also stores each player's Discord user ID together with their Attending or Nope answer for every hour of every night, still with no names.

- To delete one player's Clan Battle answers, stop the bot and run `sqlite3 data/barnacle.sqlite3 "DELETE FROM cb_marks WHERE user_id = <user ID>;"`.
- `migrations/0002_cb_attendance.down.sql` removes every season, post and answer. Back the file up before running it.
- `migrations/0003_cb_season_controls.down.sql` also loses every ping role and every ended marker. It refuses to run once two seasons in the same server share a number, which is what happens as soon as a number is reused after a season ended; resolve those rows first. Back the file up before running it.

## Clan Battle sign-ups

Create a channel that only the bot can post in, and run `/cb season start` there. The bot needs View Channel, Send Messages, Embed Links and Read Message History in that channel, all of which the invite link above already grants.

CB nights run 23:30-03:30 UTC on Wednesday, Thursday, Saturday and Sunday. For each night, the bot posts the sign-up message 24 hours ahead, closes it when the night starts, and deletes it 30 minutes after it ends. The bot has to be running for each of these steps: if it is down when a night's start passes, that night gets no sign-up post.

`/cb season start` takes an optional role to ping when a sign-up post goes up. A night is pinged once, the first time its post appears, and never again: not when the post is updated, and not when it is posted afresh after a move. If the role is not marked mentionable in Discord, the bot also needs Mention @everyone, @here and All Roles in that channel, or the ping is silent.

A season's dates have to sit within six months either side of today. Seasons run two to four months and are announced a week or two ahead, so anything wider is a typo.

Three commands change a season once it is running:
- `/cb season edit` changes a season's number, dates, codename or ping role. Its sign-up post is redrawn where it already stands; it does not move channel and does not ping again.
- `/cb season move` is run in the channel the season should post in from now on. It removes the season's posts from the old channel and posts them there instead. If any of the old posts cannot be removed, nothing moves and the season stays where it is.
- `/cb season end` stops a season at once: every sign-up post it still has is deleted from Discord and nothing more posts. If any post cannot be deleted, the season keeps running so you can fix the bot's permissions and run it again. Every answer already given stays in the database, and the season's number is free to reuse.

## License

Apache-2.0. See `LICENSE`.

## Wargaming notice

Barnacle is an unofficial fan project. It is not affiliated with, endorsed by, or supported by Wargaming. World of Warships, its ship names and its ship silhouettes are trademarks or copyrighted works of Wargaming.

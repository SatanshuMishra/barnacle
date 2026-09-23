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
   https://discord.com/oauth2/authorize?client_id=<Application ID>&scope=bot%20applications.commands&permissions=274931502608
   ```

   Open it while signed in as the application's owner, pick the server, and authorize. The permissions number grants View Channels, Send Messages, Send Messages in Threads, Embed Links, Attach Files, Read Message History, Manage Channels, Connect, Speak, Video, Use Voice Activity and Move Members. Barnacle replies to the winning message, which needs Read Message History, and rounds in threads need Send Messages in Threads. Join to Create needs Manage Channels, Connect and Move Members to open rooms and move members into them, and Speak, Video and Use Voice Activity so it can copy overwrites that name them. For a server Barnacle is already in, open the link again and authorize it, or add those permissions to Barnacle's role by hand. Keep Requires OAuth2 Code Grant off on the Bot page.
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

9. Add the Clan Battle rehearsal harness once, with the bot stopped and the database backed up as above:

   ```bash
   sqlite3 data/barnacle.sqlite3 < migrations/0004_cb_rehearsal_harness.sql
   ```

   It adds a column and a table without rebuilding anything, and carries the same run-once guard as the file before it.

10. Add the Join to Create voice rooms once, with the bot stopped and the database backed up as above:

    ```bash
    sqlite3 data/barnacle.sqlite3 < migrations/0005_voice_rooms.sql
    ```

    It adds two tables without touching the others, and carries the same run-once guard.

11. Move each season's ping role into its own table once, so a season can ping up to five roles. This rebuilds `cb_seasons`, so back the database up first as above, then, with the bot stopped:

    ```bash
    sqlite3 data/barnacle.sqlite3 < migrations/0006_cb_ping_roles.sql
    ```

    Every season keeps the role it already pinged. The file carries the same run-once guard.

12. Put the bot's token where the bot can find it. Copy `env.example` to `.env` and fill in the one value:

    ```bash
    cp env.example .env && chmod 600 .env
    ```

    `.env` is ignored by git. A `DISCORD_TOKEN` already exported in your shell wins over the file, so you can override it for one run without editing anything.

13. Start the bot:

    ```bash
    cargo run --release -p barnacle-bot
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
- `migrations/0004_cb_rehearsal_harness.down.sql` drops the rehearsal clock table and the column that marks a season as nightly. Run it only after every rehearsal season has been reset, because a nightly season read back without that column would be treated as an ordinary one.
- `migrations/0006_cb_ping_roles.down.sql` puts one ping role back on each season and keeps only its first; the others are lost. Back the file up before running it.

For Join to Create, the same database stores the server and channel IDs of each Join to Create channel together with the user ID of the admin who created it, and for each open room the user ID of the member it was opened for, until the room closes.

- `migrations/0005_voice_rooms.down.sql` removes both tables. Rooms open at that moment are no longer tracked and have to be deleted by hand.

## Clan Battle sign-ups

Create a channel that only the bot can post in, and run `/cb season start` there. The bot needs View Channel, Send Messages, Embed Links and Read Message History in that channel, all of which the invite link above already grants.

CB nights run 23:30-03:30 UTC on Wednesday, Thursday, Saturday and Sunday. For each night, the bot posts the sign-up message 24 hours ahead, closes it when the night starts, and deletes it 30 minutes after it ends. The bot has to be running for each of these steps: if it is down when a night's start passes, that night gets no sign-up post.

`/cb season start` takes up to five roles to ping when a sign-up post goes up: `ping_role`, then `ping_role_2` to `ping_role_5`. The post pings them in that order, and a role given twice is pinged once. @everyone has to be picked on its own; picking it together with named roles is refused, because it already reaches every member. A night is pinged once, the first time its post appears, and never again: not when the post is updated, and not when it is posted afresh after a move. Picking @everyone pings the whole server, once per night like any role. A role marked mentionable in Discord needs no extra bot permission. Any other role, and @everyone, also need the bot to hold Mention @everyone, @here, and All Roles in that channel, or the ping is silent. Each role is checked on its own. A ping that cannot sound is refused at `/cb season start` and `/rehearse start` when a permission override on the channel is what blocks it, and otherwise warned about in the reply to those commands and to `/cb season edit` and `/cb season move`. Each night's first post also logs whether Discord registered its pings, and logs a `signup.ping.silent` warning naming any it did not.

A season's dates have to sit within six months either side of today. Seasons run two to four months and are announced a week or two ahead, so anything wider is a typo.

Four commands change a season once it is running:
- `/cb season edit` changes a season's number, dates, codename or ping roles. Giving any of `ping_role` to `ping_role_5` replaces every role the season pinged with the ones given, and `clear_ping_role:true` stops it pinging any role; giving both is refused. Its sign-up post is redrawn where it already stands; it does not move channel and does not ping again.
- `/cb season move` is run in the channel the season should post in from now on. It removes the season's posts from the old channel and posts them there instead. If any of the old posts cannot be removed, nothing moves and the season stays where it is.
- `/cb season repost number:<n>` sends the season's open sign-up post again at the bottom of its channel, for when an admin deleted it by accident or it has scrolled out of sight. It deletes the old post if it is still there, sends the same night's post with every answer kept, and records the new post in the old one's place. Only a post still open for answers is reposted; with none open, the reply says when the next one goes up. `ping_again` is false by default, so a repost pings nobody. `ping_again:true` pings the season's roles once more, and is refused when the season has no ping role, or when the person running it could not ping those roles themselves: they need Mention @everyone, @here, and All Roles in the season's channel, or every role has to be mentionable. Like `/cb season start`, a repost with `ping_again:true` checks each role against the bot's own permissions, opens its reply with a warning for any role that cannot sound, and logs each check.
- `/cb season end` stops a season at once: every sign-up post it still has is deleted from Discord and nothing more posts. If any post cannot be deleted, the season keeps running so you can fix the bot's permissions and run it again. Every answer already given stays in the database, and the season's number is free to reuse.

### Rehearsing in a throwaway server

Every step above is driven by the clock, so a change to the sign-up flow cannot be watched on the day it is written. A rehearsal server is a Discord server that holds nothing anyone cares about, listed in `barnacle.toml` under `[rehearsal]`:

```toml
[rehearsal]
guilds = [222222222222222222]
```

A server listed there has to be in `commands.guilds` as well, which means `commands.scope` has to be `"guilds"`.

A rehearsal season is an ordinary season whose nights fall on every calendar day rather than only on CB days. Nothing else differs: each night still starts at 23:30 UTC, its post still goes up 24 hours ahead, still closes when the night starts and is still deleted 30 minutes after it ends, and the same timer that serves a real server performs every one of those steps. What sets a rehearsal server apart is a clock offset. The timer beats that server at the real time plus its offset, and `/rehearse next` moves the offset forward to the next instant at which the timer has something to do. Because every rehearsal night is followed by another the next day, each `next` after the first shows one night closing and the following night's post going up in the same beat, which is the sequence a real server only produces on a Wednesday or a Saturday.

Three commands exist only in a server the section names. The bot registers a command list per server every time it starts, so a server the section does not name is sent a list without them and they do not appear in its command list. All three also refuse to run in a server the section does not name, so the guard does not depend on the registration being right.

Taking a server out of `[rehearsal]` while leaving it in `commands.guilds` removes the commands on the next start. Removing it from `commands.guilds` as well does not: the bot never writes to a server it is not told about, so that server keeps the list it was last given until you clear its commands by hand. Take it out of `[rehearsal]`, start the bot once, and only then remove it from `commands.guilds`.

- `/rehearse start` sets up a rehearsal season that posts in this channel. It takes the season number, an optional codename, up to five ping roles as `/cb season start` does, and how many nights to run: three by default, seven at most. The nights start tomorrow, so the first post goes up a genuine 24 hours before its night rather than retroactively. Everything else is what `/cb season start` checks: a text channel, the bot's permissions there, the six-month window on the dates, and the number and overlap rules. Starting a rehearsal also resets that server's clock, so a fresh rehearsal always begins from the real time.
- `/rehearse next` moves the server's clock to the next moment at which the timer would post, close or remove a sign-up, runs that beat, and says what it did. Once the last night's post has been removed it replies that nothing is waiting.
- `/rehearse reset` deletes every Clan Battle season, sign-up post and answer in that server, including seasons that have already ended, and clears the server's clock offset. If any post cannot be removed from Discord, no season and no answer is deleted, and the reply says how many posts had already been cleared before it stopped. The silhouette game's rounds and solves are left alone.

A full rehearsal is `/rehearse start number:99`, then `/rehearse next` to watch the first post appear with its ping, some clicks, `/cb season edit`, `/cb season move` in another channel, `/rehearse next` to watch the first night close and the second night's post appear together, `/rehearse next` to watch the first post deleted, `/rehearse next` twice more to walk the second night through the same two steps, and finally `/rehearse reset`. Do not list a real clan server here: a reset deletes its seasons without asking which of them mattered.

## Join to Create voice channels

A Join to Create channel is a voice channel that hands out rooms. A member who joins it gets a new voice channel of their own, a room, and is moved into it. Several members joining at once, or one after another, each get their own room. Bots that join are ignored.

Four commands manage them. Each needs Manage Channels, and each reply is visible only to the person who ran it.

- `/voice hub create` makes a Join to Create channel. `room_name` is required and names its rooms. `name` names the channel itself and is Join to Create when left empty; `category` puts it in a category and leaves it outside any when left empty. The command sets no permissions on the channel: set who can see and join it in Discord, in the channel's permissions, because every room copies them.
- `/voice hub edit` changes the Join to Create channel given as `hub`. `name` renames it, `room_name` changes what rooms opened from now on are called, `category` moves it into a category and `top_level` moves it out of its category; give a category or top_level, not both. Rooms already open keep their names.
- `/voice hub remove` deletes the Join to Create channel given as `hub`. Rooms it opened stay until they empty, then close as usual.
- `/voice hub list` lists the server's Join to Create channels and how many rooms each has open.

A room is named after its Join to Create channel's room name plus a number, so a room name of `cb` gives `cb-1`, `cb-2` and so on. The number is one more than the highest number among rooms still open in that server under the same room name, or 1 when none is open: with cb-1, cb-2 and cb-3 open, closing cb-2 makes the next room cb-4, while closing cb-3 instead makes it cb-3. Two Join to Create channels that share a room name draw from the same numbers, so they never open two rooms with one name.

A room is deleted five minutes after its last member leaves, unless someone joins it again first. If the member leaves voice before Barnacle has moved them in, the room is deleted at once.

A room copies the Join to Create channel's category, position, permissions, bitrate, user limit, region and video quality. Barnacle also gives itself View Channel, Connect, Move Members and Manage Channels on every room, so it can move the member in and later delete the room even when the copied permissions shut its roles out.

Discord only lets a bot set an overwrite for a permission the bot itself holds. When a Join to Create channel's overwrites name a permission Barnacle's role lacks, Discord refuses the room, Barnacle logs a warning and the member stays where they are, so grant that permission to Barnacle's role. An overwrite naming Manage Permissions can only be copied by an administrator.

Barnacle only sees members join while it is running. Someone already sitting in a Join to Create channel when Barnacle starts has to leave and join it again, and a room left empty while Barnacle was stopped closes five minutes after Barnacle starts.

## Deploying to a server

The sections above cover running Barnacle on your own machine. To run it as a
container on a server, see [docs/deploy/dokploy.md](docs/deploy/dokploy.md). The
image carries the bot, the catalog and the migrations, applies pending migrations
on start, and reads its config from environment variables, so no `barnacle.toml`
is needed there.

## Logs

Barnacle logs to the console. `BARNACLE_LOG_FORMAT` picks `text` (the default) or `json`, one JSON object per line for a log shipper, and `BARNACLE_LOG_LEVEL` sets the level filter, `info` by default. Both are read from the process environment, not from `.env`. Every failure a person sees in Discord ends with a reference that finds its log event. [docs/logging.md](docs/logging.md) lists every event and field.

## License

Apache-2.0. See `LICENSE`.

## Wargaming notice

Barnacle is an unofficial fan project. It is not affiliated with, endorsed by, or supported by Wargaming. World of Warships, its ship names and its ship silhouettes are trademarks or copyrighted works of Wargaming.

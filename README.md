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

12. Add the update announcements table once, with the bot stopped and the database backed up as above:

    ```bash
    sqlite3 data/barnacle.sqlite3 < migrations/0007_bot_updates.sql
    ```

    It adds one table without touching the others, and carries the same run-once guard.

13. Turn each season into an event once, so a server can schedule events with any name, days, start time and length. This rebuilds `cb_seasons` and `cb_marks`, so back the database up first as above, then, with the bot stopped:

    ```bash
    sqlite3 data/barnacle.sqlite3 < migrations/0008_events.sql
    ```

    Every season becomes a Clan Battles event named after its number and codename, with its posts and answers kept. The file carries the same run-once guard.

14. Put the bot's token where the bot can find it. Copy `env.example` to `.env` and fill in the one value:

    ```bash
    cp env.example .env && chmod 600 .env
    ```

    `.env` is ignored by git. A `DISCORD_TOKEN` already exported in your shell wins over the file, so you can override it for one run without editing anything.

15. Start the bot:

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
- `migrations/0008_events.down.sql` gives each event its id as its season number and its name as its codename. It refuses to run while any event has a schedule other than the Clan Battles one or every day at 23:30 UTC for 4 hours, or any answer is for an hour above 4; resolve those rows first. Back the file up before running it.

For Join to Create, the same database stores the server and channel IDs of each Join to Create channel together with the user ID of the admin who created it, and for each open room the user ID of the member it was opened for, until the room closes.

- `migrations/0005_voice_rooms.down.sql` removes both tables. Rooms open at that moment are no longer tracked and have to be deleted by hand.

For update announcements, the same database stores each server's updates channel, ping role and last announced version, with no names.

- `migrations/0007_bot_updates.down.sql` removes the table. Every server has to choose its updates channel again, and the next `/announce` there posts the running version's notes again.

## Event sign-ups

Create a channel that only the bot can post in, and run `/event create` there. The bot needs View Channel, Send Messages, Embed Links and Read Message History in that channel, all of which the invite link above already grants. Every `/event` command needs Manage Server, and every reply is visible only to the person who ran it.

An event holds one session on each of its weekdays from its first day to its last. For each session, the bot posts the sign-up message 24 hours ahead, closes it when the session starts, and deletes it 30 minutes after it ends. The bot has to be running for each of these steps: if it is down when a session's start passes, that session gets no sign-up post. Every time in a sign-up post is a Discord timestamp, so each member sees it in their own time zone.

`/event create` takes:
- `name`, from 1 to 100 characters once surrounding spaces are trimmed. Two running events in one server cannot share a name, whatever its case; an event that has ended, or whose last post has been deleted, gives its name up.
- `first_day` and `last_day`, written `YYYY-MM-DD`. While you type, the bot suggests dates from yesterday to six months ahead, each with its weekday. The dates have to sit within six months either side of today, and at least one session has to be still to start.
- `template`, which fills in the start, hours and time zone. The one template, Clan Battles, runs 23:30 UTC for 4 hours on Wednesday, Thursday, Saturday and Sunday. Any of `start`, `hours` and `time_zone` given beside it replaces the template's value.
- `start`, on a 24-hour clock such as `17:00` or `5:30`, in the event's time zone.
- `hours`, from 1 to 24.
- `time_zone`, a place name such as `America/Denver` or `Europe/Berlin`, or `UTC`, picked from the suggestions. The suggestions list the zones of the server's running events first. An abbreviation such as `MDT`, an offset such as `UTC-6` and a name without a place such as `EST` are refused: they do not follow the place's own daylight saving changes, so a start written with one drifts by an hour twice a year.

Without a template, `start`, `hours` and `time_zone` are all needed, and the reply names any that were left out.

`/event create` saves nothing yet. It opens a private panel that previews the event, its first session and how many sessions it holds, with:
- a weekday menu, pre-selected with the template's weekdays, or with the first day's weekday when there is no template;
- a role menu of up to five roles to ping, empty at first;
- a Ping @everyone button, off at first;
- Create, which checks the event again and saves it, and Cancel, which closes the panel and saves nothing.

Only the admin who opened a panel can use it. A panel expires 15 minutes after it opens, and when the bot restarts; run the command again to open a new one.

The roles picked on the panel are pinged in that order when a sign-up post goes up. @everyone has its own button and cannot be picked together with roles, because it already reaches every member; turning it on while roles are picked, or picking roles while it is on, is refused. A session is pinged once, the first time its post appears, and never again: not when the post is updated, and not when it is posted afresh after a move. Picking @everyone pings the whole server, once per session like any role. A role marked mentionable in Discord needs no extra bot permission. Any other role, and @everyone, also need the bot to hold Mention @everyone, @here, and All Roles in that channel, or the ping is silent. Each role is checked on its own. A ping that cannot sound is refused at Create and at `/rehearse start` when a permission override on the channel is what blocks it, and otherwise warned about in the reply to those and to Save. Each session's first post also logs whether Discord registered its pings, and logs a `signup.ping.silent` warning naming any it did not.

An event of up to four hours shows Attend all and Nope all, then a row for each hour. A longer event shows Attend all, Nope all and Choose hours: Choose hours opens a private menu that lists each hour with its start and end, the member picks the hours they attend, and the post shows each member's hours as ranges such as `1–3, 5`.

Four more commands work on running events. Edit, repost and end each pick the event by name from a list as you type.
- `/event edit` opens the same panel, pre-selected with the event's weekdays, roles and Ping @everyone setting and showing any of `name`, `first_day`, `last_day`, `start`, `hours`, `time_zone` and `channel` given, with Save in place of Create. Save changes only what was changed on the panel or in the options, so a change another admin saved to another field in the meantime stays; with nothing changed it is refused. A change to the dates, weekdays, start time or time zone needs a session still to start; a change to only the name, hours, roles or channel does not. The posts of sessions the new schedule no longer holds are deleted, and the rest are redrawn where they stand without pinging again. Lowering the hours deletes the answers for the hours above the new count. `channel` takes a text channel of this server: the event's posts are deleted from its old channel first, and if any of them cannot be deleted, nothing changes and the event stays where it is. An event that ended, or was removed, after the panel opened is refused as not found.
- `/event repost event:<event>` sends the event's open sign-up post again at the bottom of its channel, for when an admin deleted it by accident or it has scrolled out of sight. It deletes the old post if it is still there, sends the same session's post with every answer kept, and records the new post in the old one's place. Only a post still open for answers is reposted; with none open, the reply says when the next one goes up. `ping_again` is false by default, so a repost pings nobody. `ping_again:true` pings the event's roles once more, and is refused when the event pings nobody, or when the person running it could not ping those roles themselves: they need Mention @everyone, @here, and All Roles in the event's channel, or every role has to be mentionable. Like Create, a repost with `ping_again:true` checks each role against the bot's own permissions, opens its reply with a warning for any role that cannot sound, and logs each check.
- `/event end event:<event>` stops an event at once: every sign-up post it still has is deleted from Discord and nothing more posts. If any post cannot be deleted, the event keeps running so you can fix the bot's permissions and run it again. Every answer already given stays in the database, and the event's name is free to reuse. An event that never posted is removed outright.
- `/event list` lists the server's running events, each with its channel, dates, weekdays, start time, hours, time zone, sessions still ahead and next sign-up post.

### Rehearsing in a throwaway server

Every step above is driven by the clock, so a change to the sign-up flow cannot be watched on the day it is written. A rehearsal server is a Discord server that holds nothing anyone cares about, listed in `barnacle.toml` under `[rehearsal]`:

```toml
[rehearsal]
guilds = [222222222222222222]
```

A server listed there has to be in `commands.guilds` as well, which means `commands.scope` has to be `"guilds"`.

A rehearsal event is an ordinary event whose sessions fall on every calendar day at 23:30 UTC for 4 hours. Nothing else differs: its post still goes up 24 hours ahead, still closes when the session starts and is still deleted 30 minutes after it ends, and the same timer that serves a real server performs every one of those steps. What sets a rehearsal server apart is a clock offset. The timer beats that server at the real time plus its offset, and `/rehearse next` moves the offset forward to the next instant at which the timer has something to do. Because every rehearsal session is followed by another the next day, each `next` after the first shows one session closing and the following session's post going up in the same beat, which is the sequence a Clan Battles event only produces on a Wednesday or a Saturday.

Three commands exist only in a server the section names. The bot registers a command list per server every time it starts, so a server the section does not name is sent a list without them and they do not appear in its command list. All three also refuse to run in a server the section does not name, so the guard does not depend on the registration being right.

Taking a server out of `[rehearsal]` while leaving it in `commands.guilds` removes the commands on the next start. Removing it from `commands.guilds` as well does not: the bot never writes to a server it is not told about, so that server keeps the list it was last given until you clear its commands by hand. Take it out of `[rehearsal]`, start the bot once, and only then remove it from `commands.guilds`.

- `/rehearse start` sets up a rehearsal event that posts in this channel. It takes the event's `name`, up to five roles to ping as `ping_role`, then `ping_role_2` to `ping_role_5`, and `sessions`, how many sessions to run: three by default, seven at most. The sessions start tomorrow, so the first post goes up a genuine 24 hours before its session rather than retroactively. Everything else is what `/event create` checks: a text channel, the bot's permissions there, the name rule and the ping rules. Only one rehearsal runs in a server at a time. Starting a rehearsal also resets that server's clock, so a fresh rehearsal always begins from the real time.
- `/rehearse next` moves the server's clock to the next moment at which the timer would post, close or remove a sign-up, runs that beat, and says what it did. Once the last session's post has been removed it replies that nothing is waiting.
- `/rehearse reset` deletes every event, sign-up post and answer in that server, including events that have already ended, and clears the server's clock offset. If any post cannot be removed from Discord, no event and no answer is deleted, and the reply says how many posts had already been cleared before it stopped. The silhouette game's rounds and solves are left alone.

A full rehearsal is `/rehearse start name:Drill`, then `/rehearse next` to watch the first post appear with its ping, some clicks, `/event edit` with a new name or roles, `/event edit` with `channel` set to another channel, `/rehearse next` to watch the first session close and the second session's post appear together, `/rehearse next` to watch the first post deleted, `/rehearse next` twice more to walk the second session through the same two steps, and finally `/rehearse reset`. Do not list a real clan server here: a reset deletes its events without asking which of them mattered.

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

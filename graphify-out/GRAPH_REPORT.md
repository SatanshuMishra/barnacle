# Graph Report - cb-readme  (2026-09-17)

## Corpus Check
- 90 files · ~79,795 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1203 nodes · 2846 edges · 50 communities (49 shown, 1 thin omitted)
- Extraction: 94% EXTRACTED · 6% INFERRED · 0% AMBIGUOUS · INFERRED: 164 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `9a20e97f`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_ShipIndex|ShipIndex]]
- [[_COMMUNITY_book|book]]
- [[_COMMUNITY_StoreError|StoreError]]
- [[_COMMUNITY_commands.rs|commands.rs]]
- [[_COMMUNITY_store.rs|store.rs]]
- [[_COMMUNITY_table.rs|table.rs]]
- [[_COMMUNITY_curate|curate]]
- [[_COMMUNITY_Design Barnacle's Discord bot|Design: Barnacle's Discord bot]]
- [[_COMMUNITY_Spec Barnacle's ship silhouette game|Spec: Barnacle's ship silhouette game]]
- [[_COMMUNITY_build_catalog.rs|build_catalog.rs]]
- [[_COMMUNITY_Design Clan Battle attendance sign-ups|Design: Clan Battle attendance sign-ups]]
- [[_COMMUNITY_validate.rs|validate.rs]]
- [[_COMMUNITY_solves.rs|solves.rs]]
- [[_COMMUNITY_fakes.rs|fakes.rs]]
- [[_COMMUNITY_Ship|Ship]]
- [[_COMMUNITY_String|String]]
- [[_COMMUNITY_paper_flags|paper_flags]]
- [[_COMMUNITY_ChannelId|ChannelId]]
- [[_COMMUNITY_1. How track's game worked|1. How track's game worked]]
- [[_COMMUNITY_RoundOptions|RoundOptions]]
- [[_COMMUNITY_table.rs|table.rs]]
- [[_COMMUNITY_translations.rs|translations.rs]]
- [[_COMMUNITY_UserId|UserId]]
- [[_COMMUNITY_File Structure|File Structure]]
- [[_COMMUNITY_Directory|Directory]]
- [[_COMMUNITY_solves.rs|solves.rs]]
- [[_COMMUNITY_text.rs|text.rs]]
- [[_COMMUNITY_Self|Self]]
- [[_COMMUNITY_Snowflake|Snowflake]]
- [[_COMMUNITY_config.rs|config.rs]]
- [[_COMMUNITY_CatalogRoot|CatalogRoot]]
- [[_COMMUNITY_File Structure|File Structure]]
- [[_COMMUNITY_discord.rs|discord.rs]]
- [[_COMMUNITY_File Structure|File Structure]]
- [[_COMMUNITY_startup.rs|startup.rs]]
- [[_COMMUNITY_catalog|catalog]]
- [[_COMMUNITY_Ending|Ending]]
- [[_COMMUNITY_startup.rs|startup.rs]]
- [[_COMMUNITY_round.rs|round.rs]]
- [[_COMMUNITY_events.rs|events.rs]]
- [[_COMMUNITY_main.rs|main.rs]]
- [[_COMMUNITY_RecentShips|RecentShips]]
- [[_COMMUNITY_Bot name candidates|Bot name candidates]]
- [[_COMMUNITY_Barnacle|Barnacle]]
- [[_COMMUNITY_locked_version|locked_version]]

## God Nodes (most connected - your core abstractions)
1. `ShipIndex` - 93 edges
2. `String` - 93 edges
3. `Catalog` - 51 edges
4. `curate()` - 36 edges
5. `CurationConfig` - 33 edges
6. `StoreError` - 32 edges
7. `yamato_table()` - 30 edges
8. `RoundOptions` - 29 edges
9. `book()` - 27 edges
10. `started()` - 26 edges

## Surprising Connections (you probably didn't know these)
- `the_hint_is_the_tier_when_the_range_spans_several_tiers()` --calls--> `book()`  [INFERRED]
  crates/barnacle-guess/tests/draw.rs → crates/barnacle-guess/tests/common/mod.rs
- `a_pool_ship_shows_its_details_answers_and_lookalikes()` --calls--> `ship_card()`  [INFERRED]
  crates/barnacle-bot/tests/info.rs → crates/barnacle-bot/src/info.rs
- `an_excluded_ship_says_why_and_names_its_base()` --calls--> `ship_card()`  [INFERRED]
  crates/barnacle-bot/tests/info.rs → crates/barnacle-bot/src/info.rs
- `load()` --calls--> `curate()`  [INFERRED]
  crates/barnacle-bot/src/startup.rs → crates/barnacle-catalog/src/curation/rules.rs
- `load()` --calls--> `validate()`  [INFERRED]
  crates/barnacle-bot/src/startup.rs → crates/barnacle-catalog/src/curation/validate.rs

## Import Cycles
- 1-file cycle: `crates/barnacle-bot/tests/config.rs -> crates/barnacle-bot/tests/config.rs`
- 1-file cycle: `crates/barnacle-bot/tests/text.rs -> crates/barnacle-bot/tests/text.rs`
- 1-file cycle: `crates/barnacle-bot/tests/wiring.rs -> crates/barnacle-bot/tests/wiring.rs`
- 1-file cycle: `crates/barnacle-catalog/tests/validate.rs -> crates/barnacle-catalog/tests/validate.rs`
- 1-file cycle: `crates/barnacle-data/tests/build_catalog.rs -> crates/barnacle-data/tests/build_catalog.rs`
- 1-file cycle: `crates/barnacle-catalog/tests/diff.rs -> crates/barnacle-catalog/tests/diff.rs`

## Communities (50 total, 1 thin omitted)

### Community 0 - "ShipIndex"
Cohesion: 0.06
Nodes (80): Option, Vec, ship_card(), ShipCard, field(), AliasEntry, CurationConfig, ExcludeEntry (+72 more)

### Community 1 - "book"
Cohesion: 0.16
Nodes (14): cleaned_names(), group_by_key(), BTreeMap, BTreeSet, Draw, Item, Iterator, K (+6 more)

### Community 2 - "StoreError"
Cohesion: 0.08
Nodes (47): Client, build(), Built, CatalogName, check_build_dir(), DataDir, download(), download_from() (+39 more)

### Community 3 - "commands.rs"
Cohesion: 0.10
Nodes (40): about(), all(), channel_access(), departed(), guess(), leaderboard(), leaderboard_embeds(), member_names() (+32 more)

### Community 4 - "store.rs"
Cohesion: 0.16
Nodes (22): a_failed_build_leaves_nothing_behind(), a_failed_publish_removes_its_staging_directory(), a_revision_counter_that_would_overflow_is_an_error(), built_catalogs_are_ordered_by_build_then_revision(), catalog_entries(), concurrent_builds_choose_different_revisions(), current_is_absent_until_set(), data_dir_with() (+14 more)

### Community 5 - "table.rs"
Cohesion: 0.17
Nodes (40): at(), a_busy_channel_refuses_a_second_round_until_the_first_ends(), a_cancel_while_a_win_is_settling_finds_the_round_over(), a_channel_does_not_repeat_its_recent_ships(), a_click_on_a_round_that_is_not_running_is_already_over(), a_correct_answer_after_the_settle_window_changes_nothing(), a_discord_call_that_never_finishes_is_abandoned_after_five_seconds(), a_failed_round_post_leaves_the_channel_free() (+32 more)

### Community 6 - "curate"
Cohesion: 0.11
Nodes (30): curate(), diff(), Option, catalog(), index(), Vec, ship(), a_copy_of_a_variant_points_at_the_pool_base() (+22 more)

### Community 7 - "Design: Barnacle's Discord bot"
Cohesion: 0.05
Nodes (37): 10. Testing, 11. Discord setup (owner, once), 12. Items settled while writing Plan 3, 13. Changes after the code review (2026-09-16), 14. `/leaderboard` (added after the live checklist, 2026-09-16), 1. Scope, 2. Decisions, 3. Dependencies (+29 more)

### Community 8 - "Spec: Barnacle's ship silhouette game"
Cohesion: 0.06
Nodes (34): 10. Decisions and open questions, 11.1 Why the game concept can be reused, 11.2 What must not be copied from track, 11.3 Dependencies, 11.4 Wargaming's assets and marks, 11. Licensing, 1. Goal, 2.1 Language: Rust (decided) (+26 more)

### Community 9 - "build_catalog.rs"
Cohesion: 0.07
Nodes (46): build_catalog(), BuildInputs, ExtractError, read(), read_if_present(), Error, ImageError, Option (+38 more)

### Community 10 - "Design: Clan Battle attendance sign-ups"
Cohesion: 0.06
Nodes (32): 10. The sign-up post, 11. Errors, 12. README changes, 13. What the owner reviews, not the bot, 14. Known limits, 15. Tests, 16. Work split, 1. Scope (+24 more)

### Community 11 - "validate.rs"
Cohesion: 0.10
Nodes (16): a_lookalike_naming_a_missing_ship_is_reported(), reviewed(), AppError, Cli, Command, load_curation(), main(), problems_for() (+8 more)

### Community 12 - "solves.rs"
Cohesion: 0.15
Nodes (21): GuildId, Self, Column, columns(), Profile, Duration, Error, Option (+13 more)

### Community 13 - "fakes.rs"
Cohesion: 0.14
Nodes (17): A, AtomicU64, SolveRecord, Table, FakeDiscord, FakeStore, Posted, Arc (+9 more)

### Community 14 - "Ship"
Cohesion: 0.12
Nodes (20): ModelError, Nation, ParamId, Display, Option, Ship, ShipClass, ShipGroup (+12 more)

### Community 15 - "String"
Cohesion: 0.15
Nodes (28): about(), best_time(), cancelled(), class_label(), discord_length(), empty_pool(), escape(), hint() (+20 more)

### Community 16 - "paper_flags"
Cohesion: 0.18
Nodes (21): as_dict(), is_ship(), key(), paper_flags(), PaperError, params_dict(), BTreeMap, GameDataError (+13 more)

### Community 17 - "ChannelId"
Cohesion: 0.17
Nodes (12): ChannelId, Place, Arc, Mutex, Result, Self, StartOutcome, Table<A, S> (+4 more)

### Community 18 - "1. How track's game worked"
Cohesion: 0.10
Nodes (20): 1.1 Entry point, 1.2 The eligible ship pool, and how test ships and carbon copies were excluded, 1.3 Similar-ship groups, 1.4 The silhouette image, 1.5 Answer matching, 1.6 Game flow and timing, 1.7 The companion command, `/inspect`, 1.8 How track was updated per game patch (+12 more)

### Community 19 - "RoundOptions"
Cohesion: 0.12
Nodes (12): From, Tier, u32, Draw, Hint, BTreeSet, Self, default_tier() (+4 more)

### Community 20 - "table.rs"
Cohesion: 0.20
Nodes (16): a_collaboration_reskin_without_a_twin_adds_no_names(), a_copy_of_a_variant_counts_and_a_baseless_exclusion_does_not(), a_lookalike_brings_its_variants_and_aliases(), a_lookalike_counts_only_when_its_tier_is_in_the_range(), a_name_that_cleans_to_nothing_is_not_an_answer(), a_paper_lookalike_does_not_count_in_a_historical_round(), a_ship_in_two_lookalike_groups_accepts_both_groups(), a_ships_own_names_and_its_lookalikes_can_be_listed() (+8 more)

### Community 21 - "translations.rs"
Cohesion: 0.15
Nodes (10): EnglishNames, NamesError, Error, Option, Path, PathBuf, Result, Self (+2 more)

### Community 22 - "UserId"
Cohesion: 0.12
Nodes (19): Active, CancelOutcome, Ending, Leader, Draw, Option, Seat, Duration (+11 more)

### Community 23 - "File Structure"
Cohesion: 0.11
Nodes (17): Barnacle Discord Bot Implementation Plan, Changes after the code review (2026-09-16), Changes after the live checklist (2026-09-16), File Structure, Global Constraints, How to read the file steps, Self-review, Task 10: Full verification and the owner's live checklist (+9 more)

### Community 24 - "Directory"
Cohesion: 0.19
Nodes (10): Directory, Listing, Option, Self, Vec, Suggestion, ascii_without_punctuation(), clean_answer() (+2 more)

### Community 25 - "solves.rs"
Cohesion: 0.05
Nodes (37): catalog(), curation(), fleet(), index(), memory_pool(), migrated_pool(), SqlitePool, Vec (+29 more)

### Community 26 - "text.rs"
Cohesion: 0.17
Nodes (14): fleet(), numbered(), Vec, a_pool_larger_than_twenty_skips_the_recent_ships(), a_pool_of_twenty_inside_a_larger_book_is_drawn_even_when_every_ship_is_recent(), a_pool_of_twenty_is_drawn_in_full_even_when_every_ship_is_recent(), drawn_over_many_rounds(), recent_ships_outside_the_pool_do_not_shrink_it() (+6 more)

### Community 27 - "Self"
Cohesion: 0.21
Nodes (5): Error, Formatter, Into, Result, Self

### Community 28 - "Snowflake"
Cohesion: 0.33
Nodes (9): at(), index(), paper(), BTreeSet, set(), ship(), with_full_name(), with_group() (+1 more)

### Community 29 - "config.rs"
Cohesion: 0.20
Nodes (13): CommandsFile, Config, ConfigFile, default_curation(), default_data_dir(), default_database(), position(), Option (+5 more)

### Community 30 - "CatalogRoot"
Cohesion: 0.14
Nodes (16): CatalogDirError, CatalogRoot, Error, Into, Option, Path, PathBuf, Result (+8 more)

### Community 31 - "File Structure"
Cohesion: 0.12
Nodes (15): Barnacle Game Rules Implementation Plan, Changes after the code review (2026-09-16), Decisions this plan makes (confirm before executing), File Structure, Global Constraints, Self-review, Task 1: Crate and round options, Task 2: Discord IDs and elapsed time (+7 more)

### Community 32 - "discord.rs"
Cohesion: 0.25
Nodes (13): CommandScope, Data, register(), Arc, Error, From, Http, Result (+5 more)

### Community 33 - "File Structure"
Cohesion: 0.13
Nodes (14): Barnacle Data Pipeline Implementation Plan, Execution notes (2026-09-16), File Structure, Global Constraints, Review follow-up (2026-09-16), Self-review against the spec, Task 1: Workspace and catalog model, Task 2: Name normalisation (+6 more)

### Community 34 - "startup.rs"
Cohesion: 0.27
Nodes (13): ConfigError, describe(), load(), open_solves(), read_config(), read_token(), Error, Option (+5 more)

### Community 35 - "catalog"
Cohesion: 0.22
Nodes (7): tier(), a_draw_carries_its_options_answers_and_reveal(), drawing_from_an_empty_pool_is_an_error(), the_hint_is_the_nation_when_the_range_is_a_single_tier(), a_reversed_range_is_swapped(), a_single_bound_keeps_the_other_default_and_is_swapped_when_reversed(), allows_only_tiers_inside_the_inclusive_range()

### Community 37 - "Ending"
Cohesion: 0.20
Nodes (13): cancel_row(), DiscordAnnouncer, Arc, Http, Result, Self, AnnounceError, Announcer (+5 more)

### Community 38 - "startup.rs"
Cohesion: 0.29
Nodes (9): curation_text(), a_complete_layout_loads(), a_missing_database_explains_how_to_create_it(), a_missing_silhouette_is_refused_and_named(), curation_problems_are_refused_and_listed(), Layout, no_selected_catalog_is_refused(), TempDir (+1 more)

### Community 39 - "round.rs"
Cohesion: 0.16
Nodes (9): at(), a_guess_sent_before_the_post_does_not_solve_the_round(), a_round_keeps_its_draw_invoker_and_post(), guess(), guesses_are_cleaned_before_they_are_compared(), started(), the_invoker_may_answer_their_own_round(), the_invoker_or_a_member_who_manages_messages_may_cancel() (+1 more)

### Community 40 - "events.rs"
Cohesion: 0.27
Nodes (10): ComponentInteraction, handle(), on_error(), private_followup(), Context, Error, Result, FrameworkContext (+2 more)

### Community 42 - "main.rs"
Cohesion: 0.43
Nodes (7): AppError, Cli, main(), ExitCode, PathBuf, Result, run()

### Community 45 - "RecentShips"
Cohesion: 0.25
Nodes (3): RecentShips, Self, Vec

### Community 46 - "Bot name candidates"
Cohesion: 0.29
Nodes (6): About "shrimpy" itself, Also considered, Bot name candidates, Brief, Names to avoid, Recommended (unranked)

### Community 47 - "Barnacle"
Cohesion: 0.25
Nodes (7): Barnacle, Clan Battle sign-ups, License, Player data, Running the bot, Updating game data, Wargaming notice

## Knowledge Gaps
- **150 isolated node(s):** `Updating game data`, `Player data`, `Clan Battle sign-ups`, `License`, `Wargaming notice` (+145 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **1 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ShipIndex` connect `ShipIndex` to `book`, `startup.rs`, `curate`, `build_catalog.rs`, `solves.rs`, `fakes.rs`, `Ship`, `String`, `RecentShips`, `RoundOptions`, `translations.rs`, `UserId`, `Directory`, `solves.rs`, `text.rs`, `Self`, `Snowflake`, `CatalogRoot`?**
  _High betweenness centrality (0.188) - this node is a cross-community bridge._
- **Why does `String` connect `String` to `ShipIndex`, `book`, `StoreError`, `commands.rs`, `store.rs`, `build_catalog.rs`, `validate.rs`, `solves.rs`, `Ship`, `paper_flags`, `RoundOptions`, `translations.rs`, `Directory`, `solves.rs`, `Self`, `Snowflake`, `CatalogRoot`, `discord.rs`, `startup.rs`, `Ending`, `startup.rs`, `locked_version`?**
  _High betweenness centrality (0.173) - this node is a cross-community bridge._
- **Why does `Catalog` connect `ShipIndex` to `discord.rs`, `book`, `startup.rs`, `StoreError`, `store.rs`, `startup.rs`, `curate`, `build_catalog.rs`, `Ship`, `String`, `translations.rs`, `Directory`, `Self`, `CatalogRoot`?**
  _High betweenness centrality (0.087) - this node is a cross-community bridge._
- **Are the 23 inferred relationships involving `curate()` (e.g. with `load()` and `a_pool_ship_shows_its_details_answers_and_lookalikes()`) actually correct?**
  _`curate()` has 23 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Updating game data`, `Player data`, `Clan Battle sign-ups` to the rest of the system?**
  _150 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `ShipIndex` be split into smaller, more focused modules?**
  _Cohesion score 0.0585446299732014 - nodes in this community are weakly interconnected._
- **Should `StoreError` be split into smaller, more focused modules?**
  _Cohesion score 0.08098891730605286 - nodes in this community are weakly interconnected._
# Graph Report - cb-core  (2026-09-17)

## Corpus Check
- 98 files · ~85,529 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1413 nodes · 3542 edges · 58 communities (54 shown, 4 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 180 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `9a20e97f`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_StoreError|StoreError]]
- [[_COMMUNITY_discord.rs|discord.rs]]
- [[_COMMUNITY_solves.rs|solves.rs]]
- [[_COMMUNITY_startup.rs|startup.rs]]
- [[_COMMUNITY_commands.rs|commands.rs]]
- [[_COMMUNITY_table.rs|table.rs]]
- [[_COMMUNITY_curate|curate]]
- [[_COMMUNITY_store.rs|store.rs]]
- [[_COMMUNITY_Design Barnacle's Discord bot|Design: Barnacle's Discord bot]]
- [[_COMMUNITY_Spec Barnacle's ship silhouette game|Spec: Barnacle's ship silhouette game]]
- [[_COMMUNITY_build_catalog.rs|build_catalog.rs]]
- [[_COMMUNITY_Design Clan Battle attendance sign-ups|Design: Clan Battle attendance sign-ups]]
- [[_COMMUNITY_rules.rs|rules.rs]]
- [[_COMMUNITY_validate.rs|validate.rs]]
- [[_COMMUNITY_solves.rs|solves.rs]]
- [[_COMMUNITY_UserId|UserId]]
- [[_COMMUNITY_fakes.rs|fakes.rs]]
- [[_COMMUNITY_String|String]]
- [[_COMMUNITY_paper_flags|paper_flags]]
- [[_COMMUNITY_ShipBook|ShipBook]]
- [[_COMMUNITY_ChannelId|ChannelId]]
- [[_COMMUNITY_RoundOptions|RoundOptions]]
- [[_COMMUNITY_1. How track's game worked|1. How track's game worked]]
- [[_COMMUNITY_Curated|Curated]]
- [[_COMMUNITY_table.rs|table.rs]]
- [[_COMMUNITY_params.rs|params.rs]]
- [[_COMMUNITY_translations.rs|translations.rs]]
- [[_COMMUNITY_draw.rs|draw.rs]]
- [[_COMMUNITY_book|book]]
- [[_COMMUNITY_round.rs|round.rs]]
- [[_COMMUNITY_File Structure|File Structure]]
- [[_COMMUNITY_AnnounceError|AnnounceError]]
- [[_COMMUNITY_Catalog|Catalog]]
- [[_COMMUNITY_CurationConfig|CurationConfig]]
- [[_COMMUNITY_Self|Self]]
- [[_COMMUNITY_ShipIndex|ShipIndex]]
- [[_COMMUNITY_File Structure|File Structure]]
- [[_COMMUNITY_File Structure|File Structure]]
- [[_COMMUNITY_Ship|Ship]]
- [[_COMMUNITY_tier|tier]]
- [[_COMMUNITY_Draw|Draw]]
- [[_COMMUNITY_mod.rs|mod.rs]]
- [[_COMMUNITY_report.rs|report.rs]]
- [[_COMMUNITY_Bot name candidates|Bot name candidates]]
- [[_COMMUNITY_Barnacle|Barnacle]]
- [[_COMMUNITY_.fmt|.fmt]]
- [[_COMMUNITY_.fmt|.fmt]]
- [[_COMMUNITY_startup.rs|startup.rs]]
- [[_COMMUNITY_events.rs|events.rs]]
- [[_COMMUNITY_main.rs|main.rs]]
- [[_COMMUNITY_root|root]]
- [[_COMMUNITY_.from|.from]]

## God Nodes (most connected - your core abstractions)
1. `String` - 99 edges
2. `ShipIndex` - 93 edges
3. `Catalog` - 51 edges
4. `Snowflake` - 46 edges
5. `UserId` - 37 edges
6. `curate()` - 36 edges
7. `Night` - 33 edges
8. `CurationConfig` - 33 edges
9. `StoreError` - 32 edges
10. `AttendanceError` - 31 edges

## Surprising Connections (you probably didn't know these)
- `year_suffixed_refits_are_lookalike_candidates_until_grouped()` --calls--> `catalog()`  [INFERRED]
  crates/barnacle-catalog/tests/diff.rs → crates/barnacle-catalog/tests/common/mod.rs
- `catalog_finds_ships_by_index()` --calls--> `catalog()`  [INFERRED]
  crates/barnacle-catalog/tests/model.rs → crates/barnacle-catalog/tests/common/mod.rs
- `catalog_json_with_a_bad_index_or_tier_is_rejected()` --calls--> `catalog()`  [INFERRED]
  crates/barnacle-catalog/tests/model.rs → crates/barnacle-catalog/tests/common/mod.rs
- `catalog_round_trips_through_json()` --calls--> `catalog()`  [INFERRED]
  crates/barnacle-catalog/tests/model.rs → crates/barnacle-catalog/tests/common/mod.rs
- `the_hint_is_the_tier_when_the_range_spans_several_tiers()` --calls--> `book()`  [INFERRED]
  crates/barnacle-guess/tests/draw.rs → crates/barnacle-guess/tests/common/mod.rs

## Import Cycles
- 1-file cycle: `crates/barnacle-bot/tests/wiring.rs -> crates/barnacle-bot/tests/wiring.rs`
- 1-file cycle: `crates/barnacle-bot/tests/table.rs -> crates/barnacle-bot/tests/table.rs`
- 1-file cycle: `crates/barnacle-data/tests/build_catalog.rs -> crates/barnacle-data/tests/build_catalog.rs`

## Communities (58 total, 4 thin omitted)

### Community 0 - "StoreError"
Cohesion: 0.07
Nodes (62): Client, sha256_hex(), AppError, Cli, Command, load_curation(), main(), problems_for() (+54 more)

### Community 1 - "discord.rs"
Cohesion: 0.19
Nodes (10): Directory, Listing, Option, Self, Vec, Suggestion, ascii_without_punctuation(), clean_answer() (+2 more)

### Community 2 - "solves.rs"
Cohesion: 0.06
Nodes (47): NewSeason, a_missing_table_is_named(), a_post_records_its_state_and_message(), create_season_rejects_a_duplicate_number(), create_season_rejects_an_overlapping_range(), created(), day(), end_season_removes_a_season_with_no_posts() (+39 more)

### Community 3 - "startup.rs"
Cohesion: 0.20
Nodes (13): CommandsFile, Config, ConfigFile, default_curation(), default_data_dir(), default_database(), position(), Option (+5 more)

### Community 4 - "commands.rs"
Cohesion: 0.11
Nodes (39): about(), all(), channel_access(), departed(), guess(), leaderboard(), leaderboard_embeds(), member_names() (+31 more)

### Community 5 - "table.rs"
Cohesion: 0.17
Nodes (40): at(), a_busy_channel_refuses_a_second_round_until_the_first_ends(), a_cancel_while_a_win_is_settling_finds_the_round_over(), a_channel_does_not_repeat_its_recent_ships(), a_click_on_a_round_that_is_not_running_is_already_over(), a_correct_answer_after_the_settle_window_changes_nothing(), a_discord_call_that_never_finishes_is_abandoned_after_five_seconds(), a_failed_round_post_leaves_the_channel_free() (+32 more)

### Community 6 - "curate"
Cohesion: 0.21
Nodes (21): curate(), catalog(), index(), Vec, ship(), a_copy_of_a_variant_points_at_the_pool_base(), a_ship_sharing_a_bad_silhouette_leaves_the_pool(), a_silhouette_shared_only_by_reskins_has_no_base() (+13 more)

### Community 7 - "store.rs"
Cohesion: 0.05
Nodes (62): build_catalog(), BuildInputs, ExtractError, read(), read_if_present(), Error, ImageError, Option (+54 more)

### Community 8 - "Design: Barnacle's Discord bot"
Cohesion: 0.05
Nodes (37): 10. Testing, 11. Discord setup (owner, once), 12. Items settled while writing Plan 3, 13. Changes after the code review (2026-09-16), 14. `/leaderboard` (added after the live checklist, 2026-09-16), 1. Scope, 2. Decisions, 3. Dependencies (+29 more)

### Community 9 - "Spec: Barnacle's ship silhouette game"
Cohesion: 0.06
Nodes (34): 10. Decisions and open questions, 11.1 Why the game concept can be reused, 11.2 What must not be copied from track, 11.3 Dependencies, 11.4 Wargaming's assets and marks, 11. Licensing, 1. Goal, 2.1 Language: Rust (decided) (+26 more)

### Community 10 - "build_catalog.rs"
Cohesion: 0.10
Nodes (17): parse_day(), Range, Date, Item, Iterator, Option, Self, a_season_set_up_late_starts_from_the_next_night() (+9 more)

### Community 11 - "Design: Clan Battle attendance sign-ups"
Cohesion: 0.06
Nodes (32): 10. The sign-up post, 11. Errors, 12. README changes, 13. What the owner reviews, not the bot, 14. Known limits, 15. Tests, 16. Work split, 1. Scope (+24 more)

### Community 12 - "rules.rs"
Cohesion: 0.20
Nodes (24): basic_removal(), best_base(), Candidate, Candidate<'a>, collaboration_prefixes(), final_base(), group_by(), identical_silhouettes() (+16 more)

### Community 14 - "solves.rs"
Cohesion: 0.09
Nodes (34): GuildId, Self, columns(), Profile, Duration, Error, Option, Path (+26 more)

### Community 15 - "UserId"
Cohesion: 0.05
Nodes (88): B, ColumnSpec, build_view(), Cell, Click, ClickOutcome, fresh_view(), HourTally (+80 more)

### Community 16 - "fakes.rs"
Cohesion: 0.06
Nodes (41): AtomicBool, Board, BoardError, PostTag, Box, Error, Send, Sync (+33 more)

### Community 17 - "String"
Cohesion: 0.14
Nodes (26): about(), best_time(), cancelled(), class_label(), discord_length(), empty_pool(), escape(), hint() (+18 more)

### Community 18 - "paper_flags"
Cohesion: 0.18
Nodes (21): as_dict(), is_ship(), key(), paper_flags(), PaperError, params_dict(), BTreeMap, GameDataError (+13 more)

### Community 19 - "ShipBook"
Cohesion: 0.16
Nodes (15): cleaned_names(), Entry, group_by_key(), BTreeMap, BTreeSet, Draw, Item, Iterator (+7 more)

### Community 20 - "ChannelId"
Cohesion: 0.05
Nodes (44): A, ChannelId, Place, Active, CancelOutcome, Leader, Arc, AtomicU64 (+36 more)

### Community 21 - "RoundOptions"
Cohesion: 0.18
Nodes (5): default_tier(), RoundOptions, Default, Option, Self

### Community 22 - "1. How track's game worked"
Cohesion: 0.10
Nodes (20): 1.1 Entry point, 1.2 The eligible ship pool, and how test ships and carbon copies were excluded, 1.3 Similar-ship groups, 1.4 The silhouette image, 1.5 Answer matching, 1.6 Game flow and timing, 1.7 The companion command, `/inspect`, 1.8 How track was updated per game patch (+12 more)

### Community 23 - "Curated"
Cohesion: 0.16
Nodes (33): Option, Vec, ship_card(), ShipCard, field(), CurationConfig, Curated, duplicate_excludes() (+25 more)

### Community 24 - "table.rs"
Cohesion: 0.20
Nodes (9): CatalogDirError, CatalogRoot, Error, Into, Option, Path, PathBuf, Result (+1 more)

### Community 25 - "params.rs"
Cohesion: 0.17
Nodes (11): ModelError, ParamId, ParamsError, GameDataError, Option, Result, Vec, ship_class() (+3 more)

### Community 26 - "translations.rs"
Cohesion: 0.15
Nodes (10): EnglishNames, NamesError, Error, Option, Path, PathBuf, Result, Self (+2 more)

### Community 27 - "draw.rs"
Cohesion: 0.18
Nodes (13): fleet(), numbered(), Vec, a_pool_larger_than_twenty_skips_the_recent_ships(), a_pool_of_twenty_inside_a_larger_book_is_drawn_even_when_every_ship_is_recent(), a_pool_of_twenty_is_drawn_in_full_even_when_every_ship_is_recent(), drawn_over_many_rounds(), recent_ships_outside_the_pool_do_not_shrink_it() (+5 more)

### Community 28 - "book"
Cohesion: 0.20
Nodes (16): a_collaboration_reskin_without_a_twin_adds_no_names(), a_copy_of_a_variant_counts_and_a_baseless_exclusion_does_not(), a_lookalike_brings_its_variants_and_aliases(), a_lookalike_counts_only_when_its_tier_is_in_the_range(), a_name_that_cleans_to_nothing_is_not_an_answer(), a_paper_lookalike_does_not_count_in_a_historical_round(), a_ship_in_two_lookalike_groups_accepts_both_groups(), a_ships_own_names_and_its_lookalikes_can_be_listed() (+8 more)

### Community 29 - "round.rs"
Cohesion: 0.25
Nodes (13): CommandScope, Data, register(), Arc, Error, From, Http, Result (+5 more)

### Community 30 - "File Structure"
Cohesion: 0.11
Nodes (17): Barnacle Discord Bot Implementation Plan, Changes after the code review (2026-09-16), Changes after the live checklist (2026-09-16), File Structure, Global Constraints, How to read the file steps, Self-review, Task 10: Full verification and the owner's live checklist (+9 more)

### Community 31 - "AnnounceError"
Cohesion: 0.27
Nodes (13): ConfigError, describe(), load(), open_solves(), read_config(), read_token(), Error, Option (+5 more)

### Community 32 - "Catalog"
Cohesion: 0.19
Nodes (13): CatalogDiff, diff(), LookalikeCandidate, Regrouped, Option, Vec, year_refit_candidates(), Display (+5 more)

### Community 33 - "CurationConfig"
Cohesion: 0.17
Nodes (12): AliasEntry, ExcludeEntry, ExcludeReason, KeepEntry, LookalikeGroup, Display, Error, Formatter (+4 more)

### Community 34 - "Self"
Cohesion: 0.21
Nodes (5): Error, Formatter, Into, Result, Self

### Community 35 - "ShipIndex"
Cohesion: 0.25
Nodes (3): RecentShips, Self, Vec

### Community 36 - "File Structure"
Cohesion: 0.12
Nodes (15): Barnacle Game Rules Implementation Plan, Changes after the code review (2026-09-16), Decisions this plan makes (confirm before executing), File Structure, Global Constraints, Self-review, Task 1: Crate and round options, Task 2: Discord IDs and elapsed time (+7 more)

### Community 37 - "File Structure"
Cohesion: 0.13
Nodes (14): Barnacle Data Pipeline Implementation Plan, Execution notes (2026-09-16), File Structure, Global Constraints, Review follow-up (2026-09-16), Self-review against the spec, Task 1: Workspace and catalog model, Task 2: Name normalisation (+6 more)

### Community 38 - "Ship"
Cohesion: 0.15
Nodes (7): Self, Option, Ship, ShipName, catalog_finds_ships_by_index(), catalog_json_with_a_bad_index_or_tier_is_rejected(), catalog_round_trips_through_json()

### Community 39 - "tier"
Cohesion: 0.22
Nodes (7): tier(), a_draw_carries_its_options_answers_and_reveal(), drawing_from_an_empty_pool_is_an_error(), the_hint_is_the_nation_when_the_range_is_a_single_tier(), a_reversed_range_is_swapped(), a_single_bound_keeps_the_other_default_and_is_swapped_when_reversed(), allows_only_tiers_inside_the_inclusive_range()

### Community 40 - "Draw"
Cohesion: 0.21
Nodes (11): Nation, ShipClass, Silhouette, Tier, TypedShip, Draw, Hint, BTreeSet (+3 more)

### Community 41 - "mod.rs"
Cohesion: 0.33
Nodes (9): at(), index(), paper(), BTreeSet, set(), ship(), with_full_name(), with_group() (+1 more)

### Community 42 - "report.rs"
Cohesion: 0.43
Nodes (5): a_first_diff_says_there_is_nothing_to_compare_with(), catalog(), diff_report_shows_each_new_ship_and_what_curation_did(), Vec, ship()

### Community 43 - "Bot name candidates"
Cohesion: 0.29
Nodes (6): About "shrimpy" itself, Also considered, Bot name candidates, Brief, Names to avoid, Recommended (unranked)

### Community 44 - "Barnacle"
Cohesion: 0.29
Nodes (6): Barnacle, License, Player data, Running the bot, Updating game data, Wargaming notice

### Community 52 - "startup.rs"
Cohesion: 0.29
Nodes (9): curation_text(), a_complete_layout_loads(), a_missing_database_explains_how_to_create_it(), a_missing_silhouette_is_refused_and_named(), curation_problems_are_refused_and_listed(), Layout, no_selected_catalog_is_refused(), TempDir (+1 more)

### Community 53 - "events.rs"
Cohesion: 0.27
Nodes (10): ComponentInteraction, handle(), on_error(), private_followup(), Context, Error, Result, FrameworkContext (+2 more)

### Community 54 - "main.rs"
Cohesion: 0.43
Nodes (7): AppError, Cli, main(), ExitCode, PathBuf, Result, run()

### Community 56 - "root"
Cohesion: 0.43
Nodes (7): a_missing_catalog_is_an_io_error_and_a_broken_one_a_json_error(), a_saved_catalog_loads_back(), a_silhouette_lives_in_the_catalogs_silhouettes_folder(), no_current_file_means_no_current_catalog(), root(), TempDir, the_current_file_names_the_catalog_without_surrounding_whitespace()

## Knowledge Gaps
- **150 isolated node(s):** `Removal`, `Updating game data`, `Player data`, `License`, `Wargaming notice` (+145 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **4 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `String` connect `String` to `StoreError`, `discord.rs`, `solves.rs`, `commands.rs`, `store.rs`, `rules.rs`, `validate.rs`, `UserId`, `fakes.rs`, `paper_flags`, `ShipBook`, `ChannelId`, `Curated`, `table.rs`, `params.rs`, `translations.rs`, `round.rs`, `AnnounceError`, `Catalog`, `CurationConfig`, `Self`, `Ship`, `Draw`, `mod.rs`, `startup.rs`, `.from`?**
  _High betweenness centrality (0.190) - this node is a cross-community bridge._
- **Why does `ShipIndex` connect `rules.rs` to `discord.rs`, `solves.rs`, `curate`, `store.rs`, `solves.rs`, `String`, `ShipBook`, `ChannelId`, `Curated`, `table.rs`, `params.rs`, `translations.rs`, `draw.rs`, `AnnounceError`, `Catalog`, `CurationConfig`, `Self`, `ShipIndex`, `Ship`, `Draw`, `mod.rs`?**
  _High betweenness centrality (0.147) - this node is a cross-community bridge._
- **Why does `UserId` connect `ChannelId` to `solves.rs`, `table.rs`, `solves.rs`, `UserId`, `fakes.rs`, `String`?**
  _High betweenness centrality (0.055) - this node is a cross-community bridge._
- **What connects `Removal`, `Updating game data`, `Player data` to the rest of the system?**
  _150 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `StoreError` be split into smaller, more focused modules?**
  _Cohesion score 0.06511761331038439 - nodes in this community are weakly interconnected._
- **Should `solves.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.05706760316066725 - nodes in this community are weakly interconnected._
- **Should `commands.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.10638297872340426 - nodes in this community are weakly interconnected._
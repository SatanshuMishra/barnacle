# Graph Report - cb-core  (2026-09-17)

## Corpus Check
- 90 files · ~79,604 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1202 nodes · 2845 edges · 52 communities (50 shown, 2 thin omitted)
- Extraction: 94% EXTRACTED · 6% INFERRED · 0% AMBIGUOUS · INFERRED: 164 edges (avg confidence: 0.8)
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
- 1-file cycle: `crates/barnacle-bot/tests/table.rs -> crates/barnacle-bot/tests/table.rs`
- 1-file cycle: `crates/barnacle-catalog/tests/validate.rs -> crates/barnacle-catalog/tests/validate.rs`
- 1-file cycle: `crates/barnacle-data/tests/build_catalog.rs -> crates/barnacle-data/tests/build_catalog.rs`

## Communities (52 total, 2 thin omitted)

### Community 0 - "StoreError"
Cohesion: 0.08
Nodes (49): Client, build(), Built, CatalogName, check_build_dir(), DataDir, download(), download_from() (+41 more)

### Community 1 - "discord.rs"
Cohesion: 0.05
Nodes (48): ComponentInteraction, Data, handle(), on_error(), private_followup(), Context, Error, Result (+40 more)

### Community 2 - "solves.rs"
Cohesion: 0.05
Nodes (37): catalog(), curation(), fleet(), index(), memory_pool(), migrated_pool(), SqlitePool, Vec (+29 more)

### Community 3 - "startup.rs"
Cohesion: 0.06
Nodes (43): CommandScope, CommandsFile, Config, ConfigError, ConfigFile, default_curation(), default_data_dir(), default_database() (+35 more)

### Community 4 - "commands.rs"
Cohesion: 0.10
Nodes (40): about(), all(), channel_access(), departed(), guess(), leaderboard(), leaderboard_embeds(), member_names() (+32 more)

### Community 5 - "table.rs"
Cohesion: 0.17
Nodes (40): at(), a_busy_channel_refuses_a_second_round_until_the_first_ends(), a_cancel_while_a_win_is_settling_finds_the_round_over(), a_channel_does_not_repeat_its_recent_ships(), a_click_on_a_round_that_is_not_running_is_already_over(), a_correct_answer_after_the_settle_window_changes_nothing(), a_discord_call_that_never_finishes_is_abandoned_after_five_seconds(), a_failed_round_post_leaves_the_channel_free() (+32 more)

### Community 6 - "curate"
Cohesion: 0.12
Nodes (28): curate(), catalog(), index(), Vec, ship(), a_copy_of_a_variant_points_at_the_pool_base(), a_ship_sharing_a_bad_silhouette_leaves_the_pool(), a_silhouette_shared_only_by_reskins_has_no_base() (+20 more)

### Community 7 - "store.rs"
Cohesion: 0.09
Nodes (32): composite(), ImageError, Result, Vec, sha256_hex(), encode(), Vec, transparent_pixels_take_the_background_and_opaque_pixels_stay() (+24 more)

### Community 8 - "Design: Barnacle's Discord bot"
Cohesion: 0.05
Nodes (37): 10. Testing, 11. Discord setup (owner, once), 12. Items settled while writing Plan 3, 13. Changes after the code review (2026-09-16), 14. `/leaderboard` (added after the live checklist, 2026-09-16), 1. Scope, 2. Decisions, 3. Dependencies (+29 more)

### Community 9 - "Spec: Barnacle's ship silhouette game"
Cohesion: 0.06
Nodes (34): 10. Decisions and open questions, 11.1 Why the game concept can be reused, 11.2 What must not be copied from track, 11.3 Dependencies, 11.4 Wargaming's assets and marks, 11. Licensing, 1. Goal, 2.1 Language: Rust (decided) (+26 more)

### Community 10 - "build_catalog.rs"
Cohesion: 0.12
Nodes (31): build_catalog(), BuildInputs, ExtractError, read(), read_if_present(), Error, ImageError, Option (+23 more)

### Community 11 - "Design: Clan Battle attendance sign-ups"
Cohesion: 0.06
Nodes (32): 10. The sign-up post, 11. Errors, 12. README changes, 13. What the owner reviews, not the bot, 14. Known limits, 15. Tests, 16. Work split, 1. Scope (+24 more)

### Community 12 - "rules.rs"
Cohesion: 0.18
Nodes (24): basic_removal(), best_base(), Candidate, Candidate<'a>, collaboration_prefixes(), final_base(), group_by(), identical_silhouettes() (+16 more)

### Community 13 - "validate.rs"
Cohesion: 0.10
Nodes (16): a_lookalike_naming_a_missing_ship_is_reported(), reviewed(), AppError, Cli, Command, load_curation(), main(), problems_for() (+8 more)

### Community 14 - "solves.rs"
Cohesion: 0.15
Nodes (21): GuildId, Self, Column, columns(), Profile, Duration, Error, Option (+13 more)

### Community 15 - "UserId"
Cohesion: 0.13
Nodes (13): Duration, Self, Snowflake, UserId, Draw, Guess, Round, Draw (+5 more)

### Community 16 - "fakes.rs"
Cohesion: 0.20
Nodes (13): SolveRecord, Ending, FakeDiscord, FakeStore, Posted, Arc, Duration, Mutex (+5 more)

### Community 17 - "String"
Cohesion: 0.17
Nodes (25): about(), best_time(), cancelled(), class_label(), discord_length(), escape(), hint(), leaderboard_pages() (+17 more)

### Community 18 - "paper_flags"
Cohesion: 0.18
Nodes (21): as_dict(), is_ship(), key(), paper_flags(), PaperError, params_dict(), BTreeMap, GameDataError (+13 more)

### Community 19 - "ShipBook"
Cohesion: 0.16
Nodes (15): cleaned_names(), Entry, group_by_key(), BTreeMap, BTreeSet, Draw, Item, Iterator (+7 more)

### Community 20 - "ChannelId"
Cohesion: 0.24
Nodes (8): ChannelId, Place, Arc, Result, Self, Table<A, S>, P, S

### Community 21 - "RoundOptions"
Cohesion: 0.14
Nodes (11): empty_pool(), From, Tier, u32, Self, default_tier(), RoundOptions, Option (+3 more)

### Community 22 - "1. How track's game worked"
Cohesion: 0.10
Nodes (20): 1.1 Entry point, 1.2 The eligible ship pool, and how test ships and carbon copies were excluded, 1.3 Similar-ship groups, 1.4 The silhouette image, 1.5 Answer matching, 1.6 Game flow and timing, 1.7 The companion command, `/inspect`, 1.8 How track was updated per game patch (+12 more)

### Community 23 - "Curated"
Cohesion: 0.28
Nodes (19): Curated, duplicate_excludes(), excluded_and_kept(), ineffective_keeps(), lookalike_problems(), missing_bases(), pool_state(), Problem (+11 more)

### Community 24 - "table.rs"
Cohesion: 0.16
Nodes (15): A, AtomicU64, Active, CancelOutcome, Leader, Draw, Mutex, Option (+7 more)

### Community 25 - "params.rs"
Cohesion: 0.17
Nodes (13): ModelError, ParamId, ShipClass, ParamsError, GameDataError, Option, Result, Vec (+5 more)

### Community 26 - "translations.rs"
Cohesion: 0.15
Nodes (10): EnglishNames, NamesError, Error, Option, Path, PathBuf, Result, Self (+2 more)

### Community 27 - "draw.rs"
Cohesion: 0.15
Nodes (14): fleet(), numbered(), Vec, a_pool_larger_than_twenty_skips_the_recent_ships(), a_pool_of_twenty_inside_a_larger_book_is_drawn_even_when_every_ship_is_recent(), a_pool_of_twenty_is_drawn_in_full_even_when_every_ship_is_recent(), drawn_over_many_rounds(), recent_ships_outside_the_pool_do_not_shrink_it() (+6 more)

### Community 28 - "book"
Cohesion: 0.20
Nodes (16): a_collaboration_reskin_without_a_twin_adds_no_names(), a_copy_of_a_variant_counts_and_a_baseless_exclusion_does_not(), a_lookalike_brings_its_variants_and_aliases(), a_lookalike_counts_only_when_its_tier_is_in_the_range(), a_name_that_cleans_to_nothing_is_not_an_answer(), a_paper_lookalike_does_not_count_in_a_historical_round(), a_ship_in_two_lookalike_groups_accepts_both_groups(), a_ships_own_names_and_its_lookalikes_can_be_listed() (+8 more)

### Community 29 - "round.rs"
Cohesion: 0.16
Nodes (9): at(), a_guess_sent_before_the_post_does_not_solve_the_round(), a_round_keeps_its_draw_invoker_and_post(), guess(), guesses_are_cleaned_before_they_are_compared(), started(), the_invoker_may_answer_their_own_round(), the_invoker_or_a_member_who_manages_messages_may_cancel() (+1 more)

### Community 30 - "File Structure"
Cohesion: 0.11
Nodes (17): Barnacle Discord Bot Implementation Plan, Changes after the code review (2026-09-16), Changes after the live checklist (2026-09-16), File Structure, Global Constraints, How to read the file steps, Self-review, Task 10: Full verification and the owner's live checklist (+9 more)

### Community 31 - "AnnounceError"
Cohesion: 0.20
Nodes (13): cancel_row(), DiscordAnnouncer, Arc, Http, Result, Self, AnnounceError, Announcer (+5 more)

### Community 32 - "Catalog"
Cohesion: 0.18
Nodes (15): Option, Vec, ship_card(), ShipCard, field(), diff(), Option, Catalog (+7 more)

### Community 33 - "CurationConfig"
Cohesion: 0.21
Nodes (13): AliasEntry, CurationConfig, ExcludeEntry, ExcludeReason, KeepEntry, LookalikeGroup, Display, Error (+5 more)

### Community 34 - "Self"
Cohesion: 0.21
Nodes (5): Error, Formatter, Into, Result, Self

### Community 35 - "ShipIndex"
Cohesion: 0.20
Nodes (9): CatalogDiff, LookalikeCandidate, Regrouped, Vec, year_refit_candidates(), ShipIndex, RecentShips, Self (+1 more)

### Community 36 - "File Structure"
Cohesion: 0.12
Nodes (15): Barnacle Game Rules Implementation Plan, Changes after the code review (2026-09-16), Decisions this plan makes (confirm before executing), File Structure, Global Constraints, Self-review, Task 1: Crate and round options, Task 2: Discord IDs and elapsed time (+7 more)

### Community 37 - "File Structure"
Cohesion: 0.13
Nodes (14): Barnacle Data Pipeline Implementation Plan, Execution notes (2026-09-16), File Structure, Global Constraints, Review follow-up (2026-09-16), Self-review against the spec, Task 1: Workspace and catalog model, Task 2: Name normalisation (+6 more)

### Community 38 - "Ship"
Cohesion: 0.24
Nodes (8): nation_label(), Nation, Display, Option, Ship, ShipGroup, ShipName, Silhouette

### Community 39 - "tier"
Cohesion: 0.22
Nodes (7): tier(), a_draw_carries_its_options_answers_and_reveal(), drawing_from_an_empty_pool_is_an_error(), the_hint_is_the_nation_when_the_range_is_a_single_tier(), a_reversed_range_is_swapped(), a_single_bound_keeps_the_other_default_and_is_swapped_when_reversed(), allows_only_tiers_inside_the_inclusive_range()

### Community 40 - "Draw"
Cohesion: 0.33
Nodes (3): Draw, Hint, BTreeSet

### Community 41 - "mod.rs"
Cohesion: 0.39
Nodes (8): index(), paper(), BTreeSet, set(), ship(), with_full_name(), with_group(), with_nation()

### Community 42 - "report.rs"
Cohesion: 0.43
Nodes (5): a_first_diff_says_there_is_nothing_to_compare_with(), catalog(), diff_report_shows_each_new_ship_and_what_curation_did(), Vec, ship()

### Community 43 - "Bot name candidates"
Cohesion: 0.29
Nodes (6): About "shrimpy" itself, Also considered, Bot name candidates, Brief, Names to avoid, Recommended (unranked)

### Community 44 - "Barnacle"
Cohesion: 0.29
Nodes (6): Barnacle, License, Player data, Running the bot, Updating game data, Wargaming notice

## Knowledge Gaps
- **149 isolated node(s):** `Updating game data`, `Player data`, `License`, `Wargaming notice`, `Brief` (+144 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **2 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ShipIndex` connect `ShipIndex` to `discord.rs`, `solves.rs`, `startup.rs`, `curate`, `build_catalog.rs`, `rules.rs`, `solves.rs`, `UserId`, `fakes.rs`, `String`, `ShipBook`, `RoundOptions`, `Curated`, `params.rs`, `translations.rs`, `draw.rs`, `Catalog`, `CurationConfig`, `Self`, `Ship`, `Draw`, `mod.rs`?**
  _High betweenness centrality (0.190) - this node is a cross-community bridge._
- **Why does `String` connect `String` to `StoreError`, `discord.rs`, `solves.rs`, `startup.rs`, `commands.rs`, `store.rs`, `build_catalog.rs`, `rules.rs`, `validate.rs`, `solves.rs`, `paper_flags`, `ShipBook`, `RoundOptions`, `params.rs`, `translations.rs`, `AnnounceError`, `Catalog`, `CurationConfig`, `Self`, `ShipIndex`, `Ship`, `Draw`, `mod.rs`?**
  _High betweenness centrality (0.170) - this node is a cross-community bridge._
- **Why does `Catalog` connect `Catalog` to `StoreError`, `discord.rs`, `Self`, `startup.rs`, `ShipIndex`, `curate`, `Ship`, `store.rs`, `build_catalog.rs`, `rules.rs`, `String`, `ShipBook`, `Curated`, `translations.rs`?**
  _High betweenness centrality (0.084) - this node is a cross-community bridge._
- **Are the 23 inferred relationships involving `curate()` (e.g. with `load()` and `a_pool_ship_shows_its_details_answers_and_lookalikes()`) actually correct?**
  _`curate()` has 23 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Updating game data`, `Player data`, `License` to the rest of the system?**
  _149 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `StoreError` be split into smaller, more focused modules?**
  _Cohesion score 0.07550860719874804 - nodes in this community are weakly interconnected._
- **Should `discord.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.05174825174825175 - nodes in this community are weakly interconnected._
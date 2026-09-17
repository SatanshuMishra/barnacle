use std::collections::BTreeMap;
use std::sync::Arc;

use barnacle_guess::Draw;
use barnacle_guess::Snowflake;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

use super::Context;
use super::Data;
use super::Error;
use super::announcer::cancel_row;
use crate::attendance::ClearScope;
use crate::attendance_store::CreateOutcome;
use crate::attendance_store::EditOutcome;
use crate::attendance_store::EndOutcome;
use crate::attendance_store::NewSeason;
use crate::attendance_store::Season;
use crate::attendance_store::SeasonChange;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Place;
use crate::ids::RoleId;
use crate::info;
use crate::schedule;
use crate::schedule::Night;
use crate::schedule::Range;
use crate::schedule::parse_day;
use crate::solves::Standing;
use crate::table::StartOutcome;
use crate::text;
use crate::wiring;
use crate::wiring::ChannelAccess;
use crate::wiring::LeaderboardRequest;
use crate::wiring::SortChoice;

const SILHOUETTE_FILE: &str = "silhouette.png";
const UNKNOWN_MEMBER: isize = 10007;

pub fn all() -> Vec<poise::Command<Data, Error>> {
    vec![
        poise::Command {
            description: Some("Start a round: name the ship from its silhouette".into()),
            ..guess()
        },
        poise::Command {
            description: Some("Look up ships".into()),
            subcommands: vec![poise::Command {
                description: Some("Show a ship's details and how /guess treats it".into()),
                ..ship_info()
            }],
            subcommand_required: true,
            ..ship()
        },
        poise::Command {
            description: Some("Show rounds won and best time in this server".into()),
            ..profile()
        },
        poise::Command {
            description: Some("List the top players in this server".into()),
            ..leaderboard()
        },
        poise::Command {
            description: Some("About Barnacle and its ship data".into()),
            ..about()
        },
        poise::Command {
            description: Some("Clan Battle sign-ups".into()),
            subcommands: vec![poise::Command {
                description: Some("Clan Battle seasons for this server".into()),
                subcommands: vec![
                    poise::Command {
                        description: Some(
                            "Set up a season and post its sign-ups in this channel".into(),
                        ),
                        ..season_start()
                    },
                    poise::Command {
                        description: Some(
                            "Change a season's number, dates, codename or ping role".into(),
                        ),
                        ..season_edit()
                    },
                    poise::Command {
                        description: Some("Move a season's sign-up posts to this channel".into()),
                        ..season_move()
                    },
                    poise::Command {
                        description: Some("End a season now and clear its sign-up posts".into()),
                        ..season_end()
                    },
                    poise::Command {
                        description: Some(
                            "List this server's seasons and when each posts next".into(),
                        ),
                        ..season_show()
                    },
                ],
                subcommand_required: true,
                ..season()
            }],
            subcommand_required: true,
            ..cb()
        },
    ]
}

fn place(ctx: Context<'_>) -> Option<Place> {
    let guild = ctx.guild_id()?;
    Some(Place {
        guild: GuildId::new(guild.get()),
        channel: ChannelId::new(ctx.channel_id().get()),
    })
}

async fn private(ctx: Context<'_>, content: impl Into<String>) -> Result<(), Error> {
    ctx.send(
        poise::CreateReply::default()
            .content(content)
            .allowed_mentions(serenity::CreateAllowedMentions::new())
            .ephemeral(true),
    )
    .await?;
    Ok(())
}

fn channel_access(ctx: Context<'_>) -> ChannelAccess {
    let (permissions, channel) = match ctx {
        poise::Context::Application(app) => (
            app.interaction.app_permissions,
            app.interaction.channel.as_ref(),
        ),
        poise::Context::Prefix(_) => (None, None),
    };
    let in_thread = channel.is_some_and(|channel| {
        matches!(
            channel.kind,
            serenity::ChannelType::PublicThread
                | serenity::ChannelType::PrivateThread
                | serenity::ChannelType::NewsThread
        )
    });
    let send = if in_thread {
        serenity::Permissions::SEND_MESSAGES_IN_THREADS
    } else {
        serenity::Permissions::SEND_MESSAGES
    };
    permissions.map_or(
        ChannelAccess {
            view_channel: true,
            send_messages: true,
            embed_links: true,
            attach_files: true,
            read_message_history: true,
            in_thread,
        },
        |granted| ChannelAccess {
            view_channel: granted.contains(serenity::Permissions::VIEW_CHANNEL),
            send_messages: granted.contains(send),
            embed_links: granted.contains(serenity::Permissions::EMBED_LINKS),
            attach_files: granted.contains(serenity::Permissions::ATTACH_FILES),
            read_message_history: granted.contains(serenity::Permissions::READ_MESSAGE_HISTORY),
            in_thread,
        },
    )
}

fn channel_kind(ctx: Context<'_>) -> Option<serenity::ChannelType> {
    match ctx {
        poise::Context::Application(app) => {
            app.interaction.channel.as_ref().map(|channel| channel.kind)
        }
        poise::Context::Prefix(_) => None,
    }
}

fn next_post_at(range: Range, now_unix: i64) -> Option<i64> {
    range
        .due_night(now_unix)
        .is_none()
        .then(|| range.next_night(now_unix).map(Night::post_at_unix))
        .flatten()
}

async fn remove_round_post(ctx: Context<'_>) {
    if let poise::Context::Application(app) = ctx
        && let Err(error) = app.interaction.delete_response(ctx.http()).await
    {
        tracing::error!(%error, "a round post that timed out could not be removed");
    }
}

fn round_embed(draw: &Draw) -> serenity::CreateEmbed {
    let embed = serenity::CreateEmbed::new()
        .title(text::ROUND_TITLE)
        .description(text::ROUND_DESCRIPTION)
        .colour(text::EMBED_COLOUR)
        .field(text::TIERS_FIELD, text::tier_range(draw.options()), true)
        .image(format!("attachment://{SILHOUETTE_FILE}"))
        .footer(serenity::CreateEmbedFooter::new(text::round_footer(
            Timing::STANDARD,
        )));
    if draw.options().historical() {
        embed.field(text::PAPER_EXCLUDED, "Yes", true)
    } else {
        embed
    }
}

#[poise::command(slash_command, guild_only)]
async fn guess(
    ctx: Context<'_>,
    #[description = "Lowest tier, 1 to 11 (default 6)"]
    #[min = 1]
    #[max = 11]
    min_tier: Option<i64>,
    #[description = "Highest tier, 1 to 11 (default 11)"]
    #[min = 1]
    #[max = 11]
    max_tier: Option<i64>,
    #[description = "Leave out ships that were never built"] historical: Option<bool>,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    let missing = wiring::missing_permissions(channel_access(ctx));
    if !missing.is_empty() {
        return private(ctx, text::missing_permissions(&missing)).await;
    }
    let data = ctx.data();
    let options = wiring::round_options(min_tier, max_tier, historical);
    let invoker = UserId::new(ctx.author().id.get());
    let outcome = data
        .table
        .start(place, options, invoker, |draw, number| async move {
            let path = data.root.silhouette(&data.catalog_name, draw.ship());
            let png = tokio::fs::read(&path).await?;
            let reply = poise::CreateReply::default()
                .embed(round_embed(&draw))
                .attachment(serenity::CreateAttachment::bytes(png, SILHOUETTE_FILE))
                .components(vec![cancel_row(wiring::cancel_button_id(number), false)]);
            let handle = ctx.send(reply).await?;
            let posted = handle.message().await.map(|message| message.id);
            match posted {
                Ok(id) => Ok::<Snowflake, Error>(Snowflake::new(id.get())),
                Err(error) => {
                    if let Err(cleanup) = handle.delete(ctx).await {
                        tracing::error!(%cleanup, "an untracked round post could not be removed");
                    }
                    Err(error.into())
                }
            }
        })
        .await;
    match outcome {
        StartOutcome::Started { .. } => Ok(()),
        StartOutcome::Busy => private(ctx, text::ALREADY_RUNNING).await,
        StartOutcome::NoShips(_) => private(ctx, text::empty_pool(&options)).await,
        StartOutcome::PostFailed(error) => Err(error),
        StartOutcome::PostTimedOut => {
            remove_round_post(ctx).await;
            Err("posting the round timed out".into())
        }
    }
}

#[poise::command(slash_command)]
async fn ship(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

async fn suggest_ships(ctx: Context<'_>, partial: &str) -> serenity::CreateAutocompleteResponse {
    let choices = ctx
        .data()
        .directory
        .suggest(partial)
        .into_iter()
        .map(|suggestion| {
            serenity::AutocompleteChoice::new(suggestion.label, suggestion.index.to_string())
        })
        .collect();
    serenity::CreateAutocompleteResponse::new().set_choices(choices)
}

#[poise::command(slash_command, rename = "info")]
async fn ship_info(
    ctx: Context<'_>,
    #[description = "Ship name"]
    #[autocomplete = "suggest_ships"]
    ship: String,
) -> Result<(), Error> {
    let data = ctx.data();
    let card = data.directory.resolve(&ship).and_then(|index| {
        info::ship_card(&data.catalog, &data.curated, data.table.book(), index)
            .map(|card| (index.clone(), card))
    });
    let Some((index, card)) = card else {
        return private(ctx, text::NO_SHIP_MATCHES).await;
    };
    let embed = card.fields.into_iter().fold(
        serenity::CreateEmbed::new()
            .title(card.title)
            .colour(text::EMBED_COLOUR),
        |embed, (name, value)| embed.field(name, value, true),
    );
    let path = data.root.silhouette(&data.catalog_name, &index);
    let reply = match tokio::fs::read(&path).await {
        Ok(png) => poise::CreateReply::default()
            .embed(embed.image(format!("attachment://{SILHOUETTE_FILE}")))
            .attachment(serenity::CreateAttachment::bytes(png, SILHOUETTE_FILE)),
        Err(_) => poise::CreateReply::default().embed(embed),
    };
    ctx.send(reply.ephemeral(true)).await?;
    Ok(())
}

#[poise::command(slash_command, guild_only)]
async fn profile(
    ctx: Context<'_>,
    #[description = "Whose profile to show (default: you)"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    let target = user.as_ref().unwrap_or_else(|| ctx.author());
    let stats = ctx
        .data()
        .table
        .solves()
        .profile(place.guild, UserId::new(target.id.get()))
        .await?;
    let embed = serenity::CreateEmbed::new()
        .title(target.display_name())
        .colour(text::EMBED_COLOUR)
        .field(text::ROUNDS_WON, stats.wins.to_string(), true)
        .field(text::BEST_TIME, text::best_time(stats.best), true);
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

fn departed(error: &serenity::Error) -> bool {
    matches!(
        error,
        serenity::Error::Http(serenity::HttpError::UnsuccessfulRequest(response))
            if response.status_code == serenity::StatusCode::NOT_FOUND
                && response.error.code == UNKNOWN_MEMBER
    )
}

async fn member_names(
    ctx: Context<'_>,
    guild: GuildId,
    standings: &[Standing],
) -> Result<Vec<String>, Error> {
    let guild = serenity::GuildId::new(guild.get());
    let lookups: tokio::task::JoinSet<_> = standings
        .iter()
        .enumerate()
        .map(|(position, standing)| {
            let http = Arc::clone(&ctx.serenity_context().http);
            let permits = Arc::clone(&ctx.data().member_lookups);
            let user = serenity::UserId::new(standing.user.get());
            async move {
                let _permit = permits.acquire_owned().await;
                (position, http.get_member(guild, user).await)
            }
        })
        .collect();
    let found: BTreeMap<usize, Result<serenity::Member, serenity::Error>> =
        lookups.join_all().await.into_iter().collect();
    found
        .into_values()
        .map(|lookup| match lookup {
            Ok(member) => Ok(member.display_name().to_owned()),
            Err(error) if departed(&error) => Ok(text::FORMER_MEMBER.to_owned()),
            Err(error) => Err(error.into()),
        })
        .collect()
}

#[poise::command(slash_command, guild_only)]
async fn leaderboard(
    ctx: Context<'_>,
    #[description = "Order by most wins or fastest time (default: most wins)"] sort: Option<
        SortChoice,
    >,
    #[description = "How many players to list (default 10)"]
    #[choices(5, 10, 15, 20, 25, 30, 35, 40, 45, 50)]
    limit: Option<u32>,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    let poise::Context::Application(app) = ctx else {
        return Ok(());
    };
    let request = wiring::leaderboard_request(sort, limit);
    ctx.defer().await?;
    let response = match leaderboard_embeds(ctx, place, request).await {
        Ok(embeds) => serenity::EditInteractionResponse::new().embeds(embeds),
        Err(error) => {
            tracing::error!(command = %ctx.command().qualified_name, %error, "a command failed");
            serenity::EditInteractionResponse::new().content(text::SOMETHING_WENT_WRONG)
        }
    };
    app.interaction.edit_response(ctx.http(), response).await?;
    Ok(())
}

async fn leaderboard_embeds(
    ctx: Context<'_>,
    place: Place,
    request: LeaderboardRequest,
) -> Result<Vec<serenity::CreateEmbed>, Error> {
    let standings = ctx
        .data()
        .table
        .solves()
        .leaderboard(place.guild, request.ranking, request.size)
        .await?;
    let names = member_names(ctx, place.guild, &standings).await?;
    let lines: Vec<String> = standings
        .iter()
        .zip(&names)
        .zip(1..)
        .map(|((standing, name), rank)| text::standing_line(rank, name, standing))
        .collect();
    let pages = match text::leaderboard_pages(&lines) {
        pages if pages.is_empty() => vec![text::NO_WINS_HERE.to_owned()],
        pages => pages,
    };
    Ok(pages
        .into_iter()
        .enumerate()
        .map(|(position, page)| {
            let embed = serenity::CreateEmbed::new()
                .colour(text::EMBED_COLOUR)
                .description(page);
            if position == 0 {
                embed.title(text::leaderboard_title(request.ranking))
            } else {
                embed
            }
        })
        .collect())
}

#[poise::command(slash_command)]
async fn about(ctx: Context<'_>) -> Result<(), Error> {
    let data = ctx.data();
    let embed = serenity::CreateEmbed::new()
        .title(text::ABOUT_TITLE)
        .colour(text::EMBED_COLOUR)
        .description(text::about(
            &data.catalog.provenance,
            &data.catalog_name,
            env!("CARGO_PKG_VERSION"),
        ));
    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "MANAGE_GUILD",
    required_permissions = "MANAGE_GUILD"
)]
async fn cb(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
async fn season(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, rename = "start")]
async fn season_start(
    ctx: Context<'_>,
    #[description = "Which CB season this is, for example 35"]
    #[min = 1]
    number: u32,
    #[description = "First CB day, as 2026-09-16"] first_day: String,
    #[description = "Last CB day, as 2026-11-05"] last_day: String,
    #[description = "The season's codename, for example Komodo Dragon"]
    #[max_length = 100]
    codename: Option<String>,
    #[description = "Role to ping when a sign-up is posted"] ping_role: Option<serenity::RoleId>,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    if channel_kind(ctx) != Some(serenity::ChannelType::Text) {
        return private(ctx, text::RUN_IN_TEXT_CHANNEL).await;
    }
    let missing = wiring::missing_signup_permissions(channel_access(ctx));
    if !missing.is_empty() {
        return private(ctx, text::missing_permissions(&missing)).await;
    }
    let (Some(first), Some(last)) = (parse_day(&first_day), parse_day(&last_day)) else {
        return private(ctx, text::DATE_FORMAT).await;
    };
    let Some(range) = Range::new(first, last) else {
        return private(ctx, text::LAST_DAY_BEFORE_FIRST).await;
    };
    let now_unix = super::now_unix();
    let Some(today) = schedule::today(now_unix) else {
        return private(ctx, text::SOMETHING_WENT_WRONG).await;
    };
    if !range.near(today) {
        return private(ctx, text::RANGE_TOO_FAR).await;
    }
    let nights_left = range.nights_left(now_unix);
    if nights_left == 0 {
        return private(ctx, text::NO_NIGHTS_LEFT).await;
    }
    let Some(created_at_ms) = super::now_ms() else {
        return private(ctx, text::SOMETHING_WENT_WRONG).await;
    };
    let new = NewSeason {
        guild: place.guild,
        channel: place.channel,
        number,
        codename,
        range,
        created_by: UserId::new(ctx.author().id.get()),
        created_at_ms,
        ping_role: ping_role.map(|role| RoleId::new(role.get())),
    };
    match ctx.data().signups.store().create_season(&new).await? {
        CreateOutcome::Created(season) => {
            let started = text::season_started(&season, nights_left, next_post_at(range, now_unix));
            private(ctx, started).await
        }
        CreateOutcome::NumberTaken => private(ctx, text::season_number_taken(number)).await,
        CreateOutcome::Overlaps(other) => private(ctx, text::season_overlaps(&other)).await,
    }
}

#[poise::command(slash_command, rename = "show")]
async fn season_show(ctx: Context<'_>) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    let now_unix = super::now_unix();
    let seasons = ctx.data().signups.store().seasons_in(place.guild).await?;
    let lines: Vec<String> = seasons
        .iter()
        .filter(|season| {
            season
                .range
                .last_moment_unix()
                .is_some_and(|last| last > now_unix)
        })
        .map(|season| {
            text::season_line(
                season,
                season.range.nights_left(now_unix),
                next_post_at(season.range, now_unix),
            )
        })
        .collect();
    if lines.is_empty() {
        return private(ctx, text::NO_SEASON_HERE).await;
    }
    private(ctx, text::season_list(&lines)).await
}

#[poise::command(slash_command, rename = "edit")]
#[allow(clippy::too_many_arguments)]
async fn season_edit(
    ctx: Context<'_>,
    #[description = "Which season to change, for example 35"]
    #[min = 1]
    number: u32,
    #[description = "Change the season number"]
    #[min = 1]
    new_number: Option<u32>,
    #[description = "New first CB day, as 2026-09-16"] first_day: Option<String>,
    #[description = "New last CB day, as 2026-11-05"] last_day: Option<String>,
    #[description = "New codename, for example Komodo Dragon"]
    #[max_length = 100]
    codename: Option<String>,
    #[description = "Role to ping when a sign-up is posted"] ping_role: Option<serenity::RoleId>,
    #[description = "Remove the codename"] clear_codename: Option<bool>,
    #[description = "Stop pinging a role"] clear_ping_role: Option<bool>,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    let clearing_codename = clear_codename.unwrap_or(false);
    let clearing_ping_role = clear_ping_role.unwrap_or(false);
    let named = new_number.is_some()
        || first_day.is_some()
        || last_day.is_some()
        || codename.is_some()
        || ping_role.is_some()
        || clearing_codename
        || clearing_ping_role;
    if !named {
        return private(ctx, text::NOTHING_TO_CHANGE).await;
    }
    if codename.is_some() && clearing_codename {
        return private(ctx, text::CODENAME_BOTH_WAYS).await;
    }
    if ping_role.is_some() && clearing_ping_role {
        return private(ctx, text::PING_BOTH_WAYS).await;
    }
    let first = first_day.as_deref().map(parse_day);
    let last = last_day.as_deref().map(parse_day);
    if matches!(first, Some(None)) || matches!(last, Some(None)) {
        return private(ctx, text::DATE_FORMAT).await;
    }
    let change = SeasonChange {
        number: new_number,
        first_day: first.flatten(),
        last_day: last.flatten(),
        codename: if clearing_codename {
            Some(None)
        } else {
            codename.map(Some)
        },
        ping_role: if clearing_ping_role {
            Some(None)
        } else {
            ping_role.map(|role| Some(RoleId::new(role.get())))
        },
    };
    let now_unix = super::now_unix();
    let Some(today) = schedule::today(now_unix) else {
        return private(ctx, text::SOMETHING_WENT_WRONG).await;
    };
    let outcome = ctx
        .data()
        .signups
        .store()
        .edit_season(place.guild, number, &change, today)
        .await?;
    match outcome {
        EditOutcome::Edited { after, .. } => {
            let signups = &ctx.data().signups;
            ctx.defer_ephemeral().await?;
            let cleared = signups
                .clear_posts(&after, ClearScope::OutsideRange, now_unix)
                .await;
            let refreshed = signups.refresh_posts(&after, now_unix).await;
            if cleared.failures > 0 || refreshed.failures > 0 {
                tracing::warn!(
                    season = after.id,
                    cleared = cleared.failures,
                    refreshed = refreshed.failures,
                    "a season edit could not reach every sign-up post"
                );
            }
            let mut edited = text::season_edited(
                &after,
                after.range.nights_left(now_unix),
                next_post_at(after.range, now_unix),
                cleared.touched,
            );
            if refreshed.failures > 0 {
                edited.push_str(text::REFRESH_FAILED);
            }
            private(ctx, edited).await
        }
        EditOutcome::NumberTaken => {
            private(ctx, text::season_number_taken(new_number.unwrap_or(number))).await
        }
        EditOutcome::Overlaps(other) => private(ctx, text::season_overlaps(&other)).await,
        EditOutcome::BadRange => private(ctx, text::LAST_DAY_BEFORE_FIRST).await,
        EditOutcome::TooFar => private(ctx, text::RANGE_TOO_FAR).await,
        EditOutcome::NotFound => private(ctx, text::season_not_found(number)).await,
    }
}

#[poise::command(slash_command, rename = "move")]
async fn season_move(
    ctx: Context<'_>,
    #[description = "Which season to move here, for example 35"]
    #[min = 1]
    number: u32,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    if channel_kind(ctx) != Some(serenity::ChannelType::Text) {
        return private(ctx, text::RUN_IN_TEXT_CHANNEL).await;
    }
    let missing = wiring::missing_signup_permissions(channel_access(ctx));
    if !missing.is_empty() {
        return private(ctx, text::missing_permissions(&missing)).await;
    }
    let signups = &ctx.data().signups;
    let Some(season) = signups.store().live_season(place.guild, number).await? else {
        return private(ctx, text::season_not_found(number)).await;
    };
    if season.channel == place.channel {
        return private(ctx, text::season_already_here(number)).await;
    }
    let now_unix = super::now_unix();
    ctx.defer_ephemeral().await?;
    let cleared = signups
        .clear_posts(&season, ClearScope::All, now_unix)
        .await;
    if cleared.failures > 0 {
        tracing::warn!(
            season = season.id,
            failures = cleared.failures,
            "a season move left sign-up posts in the old channel"
        );
        return private(ctx, text::CLEAR_FAILED_MOVE).await;
    }
    if !signups
        .store()
        .move_season(place.guild, season.id, place.channel)
        .await?
    {
        return private(ctx, text::season_not_found(number)).await;
    }
    let after = Season {
        channel: place.channel,
        ..season
    };
    let moved = text::season_moved(
        &after,
        after.range.nights_left(now_unix),
        cleared.touched,
        next_post_at(after.range, now_unix),
    );
    private(ctx, moved).await
}

#[poise::command(slash_command, rename = "end")]
async fn season_end(
    ctx: Context<'_>,
    #[description = "Which season to end, for example 35"]
    #[min = 1]
    number: u32,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    let Some(now_ms) = super::now_ms() else {
        return private(ctx, text::SOMETHING_WENT_WRONG).await;
    };
    let now_unix = super::now_unix();
    let signups = &ctx.data().signups;
    let Some(season) = signups.store().live_season(place.guild, number).await? else {
        return private(ctx, text::season_not_found(number)).await;
    };
    ctx.defer_ephemeral().await?;
    let cleared = signups
        .clear_posts(&season, ClearScope::All, now_unix)
        .await;
    if cleared.failures > 0 {
        tracing::warn!(
            season = season.id,
            failures = cleared.failures,
            "a season could not be ended because its sign-up posts remain"
        );
        return private(ctx, text::CLEAR_FAILED_END).await;
    }
    match signups
        .store()
        .end_season(place.guild, number, now_ms)
        .await?
    {
        EndOutcome::Removed => private(ctx, text::season_removed(number)).await,
        EndOutcome::Ended { .. } => private(ctx, text::season_ended(number, cleared.touched)).await,
        EndOutcome::NotFound => private(ctx, text::season_not_found(number)).await,
    }
}

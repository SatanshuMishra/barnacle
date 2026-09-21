use std::collections::BTreeMap;
use std::sync::Arc;

use barnacle_guess::Draw;
use barnacle_guess::Snowflake;
use barnacle_guess::Timing;
use barnacle_guess::UserId;
use jiff::ToSpan;
use jiff::civil::Date;
use poise::serenity_prelude as serenity;

use super::Context;
use super::Data;
use super::Error;
use super::announcer::cancel_row;
use super::rooms::MISSING_PERMISSIONS;
use super::rooms::UNKNOWN_CHANNEL;
use super::rooms::refused;
use crate::attendance::ClearScope;
use crate::attendance::next_moment;
use crate::attendance_store::Attendance;
use crate::attendance_store::CreateOutcome;
use crate::attendance_store::EditOutcome;
use crate::attendance_store::EndOutcome;
use crate::attendance_store::NewSeason;
use crate::attendance_store::Post;
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
use crate::voice::HUB_NAME_LIMIT;
use crate::voice::NameProblem;
use crate::voice::ROOM_NAME_LIMIT;
use crate::voice::clean_name;
use crate::voice_store::Hub;
use crate::voice_text;
use crate::wiring;
use crate::wiring::ChannelAccess;
use crate::wiring::LeaderboardRequest;
use crate::wiring::SortChoice;

const SILHOUETTE_FILE: &str = "silhouette.png";
const UNKNOWN_MEMBER: isize = 10007;
const MILLIS_PER_SECOND: u64 = 1000;
const REHEARSAL_NIGHTS: u32 = 3;
const REHEARSAL_NIGHTS_MAX: u32 = 7;
const CODENAME_LIMIT: usize = 100;

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
        poise::Command {
            description: Some("Join to Create voice channels".into()),
            subcommands: vec![poise::Command {
                description: Some("Join to Create channels in this server".into()),
                subcommands: vec![
                    poise::Command {
                        description: Some("Create a Join to Create voice channel".into()),
                        ..hub_create()
                    },
                    poise::Command {
                        description: Some(
                            "Change a Join to Create channel's name, room names or category".into(),
                        ),
                        ..hub_edit()
                    },
                    poise::Command {
                        description: Some("Delete a Join to Create channel".into()),
                        ..hub_remove()
                    },
                    poise::Command {
                        description: Some("List this server's Join to Create channels".into()),
                        ..hub_list()
                    },
                ],
                subcommand_required: true,
                ..hub()
            }],
            subcommand_required: true,
            ..voice()
        },
    ]
}

pub fn rehearsal() -> Vec<poise::Command<Data, Error>> {
    vec![poise::Command {
        description: Some("Drive a Clan Battle rehearsal in this server".into()),
        subcommands: vec![
            poise::Command {
                description: Some(
                    "Set up a rehearsal season of nightly sign-ups, starting tomorrow".into(),
                ),
                ..rehearse_start()
            },
            poise::Command {
                description: Some(
                    "Move this server's clock to the next sign-up step and run it".into(),
                ),
                ..rehearse_next()
            },
            poise::Command {
                description: Some(
                    "Delete every Clan Battle season, post and answer in this server".into(),
                ),
                ..rehearse_reset()
            },
        ],
        subcommand_required: true,
        ..rehearse()
    }]
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
        codename: clipped_codename(codename),
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

#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "MANAGE_GUILD",
    required_permissions = "MANAGE_GUILD"
)]
async fn rehearse(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

fn rehearsing(ctx: Context<'_>, guild: GuildId) -> bool {
    ctx.data().rehearsal.contains(&guild)
}

fn clipped_codename(codename: Option<String>) -> Option<String> {
    codename.map(|text| text.chars().take(CODENAME_LIMIT).collect())
}

fn rehearsal_range(today: Date, nights: u32, now_unix: i64) -> Option<Range> {
    let nights = nights.clamp(1, REHEARSAL_NIGHTS_MAX);
    let mut first = today.checked_add(1.day()).ok()?;
    if Night::on(first).is_some_and(|night| night.post_at_unix() <= now_unix) {
        first = first.checked_add(1.day()).ok()?;
    }
    let last = first
        .checked_add(i64::from(nights.saturating_sub(1)).days())
        .ok()?;
    Range::every_day(first, last)
}

async fn posts_of(store: &Attendance, seasons: &[Season]) -> Result<Vec<(i64, Vec<Post>)>, Error> {
    let mut posts = Vec::with_capacity(seasons.len());
    for season in seasons {
        posts.push((season.id, store.posts(season.id).await?));
    }
    Ok(posts)
}

#[poise::command(slash_command, rename = "start")]
async fn rehearse_start(
    ctx: Context<'_>,
    #[description = "Which CB season this is, for example 99"]
    #[min = 1]
    number: u32,
    #[description = "The season's codename, for example Komodo Dragon"]
    #[max_length = 100]
    codename: Option<String>,
    #[description = "Role to ping when a sign-up is posted"] ping_role: Option<serenity::RoleId>,
    #[description = "How many nights to rehearse, one per day from tomorrow (default 3)"]
    #[min = 1]
    #[max = 7]
    nights: Option<u32>,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    if !rehearsing(ctx, place.guild) {
        return private(ctx, text::REHEARSAL_ONLY).await;
    }
    if channel_kind(ctx) != Some(serenity::ChannelType::Text) {
        return private(ctx, text::RUN_IN_TEXT_CHANNEL).await;
    }
    let missing = wiring::missing_signup_permissions(channel_access(ctx));
    if !missing.is_empty() {
        return private(ctx, text::missing_permissions(&missing)).await;
    }
    let now_unix = super::now_unix();
    let Some(today) = schedule::today(now_unix) else {
        return private(ctx, text::SOMETHING_WENT_WRONG).await;
    };
    let Some(range) = rehearsal_range(today, nights.unwrap_or(REHEARSAL_NIGHTS), now_unix) else {
        return private(ctx, text::SOMETHING_WENT_WRONG).await;
    };
    if !range.near(today) {
        return private(ctx, text::RANGE_TOO_FAR).await;
    }
    let nights_left = range.nights_left(now_unix);
    let Some(created_at_ms) = super::now_ms() else {
        return private(ctx, text::SOMETHING_WENT_WRONG).await;
    };
    let new = NewSeason {
        guild: place.guild,
        channel: place.channel,
        number,
        codename: clipped_codename(codename),
        range,
        created_by: UserId::new(ctx.author().id.get()),
        created_at_ms,
        ping_role: ping_role.map(|role| RoleId::new(role.get())),
    };
    match ctx.data().signups.store().create_season(&new).await? {
        CreateOutcome::Created(season) => {
            let started =
                text::rehearsal_started(&season, nights_left, next_post_at(range, now_unix));
            private(ctx, started).await
        }
        CreateOutcome::NumberTaken => private(ctx, text::season_number_taken(number)).await,
        CreateOutcome::Overlaps(other) => private(ctx, text::season_overlaps(&other)).await,
    }
}

#[poise::command(slash_command, rename = "next")]
async fn rehearse_next(ctx: Context<'_>) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    if !rehearsing(ctx, place.guild) {
        return private(ctx, text::REHEARSAL_ONLY).await;
    }
    let signups = &ctx.data().signups;
    let store = signups.store();
    let now_unix = super::now_unix();
    let offset = store.rehearsal_clock(place.guild).await?;
    let Some(rehearsal_now) = now_unix.checked_add(offset) else {
        return private(ctx, text::SOMETHING_WENT_WRONG).await;
    };
    let seasons = store.seasons_in(place.guild).await?;
    let posts = posts_of(store, &seasons).await?;
    let Some(moment) = next_moment(&seasons, &posts, rehearsal_now) else {
        return private(ctx, text::NOTHING_PENDING).await;
    };
    let Some(moment_ms) = u64::try_from(moment)
        .ok()
        .and_then(|seconds| seconds.checked_mul(MILLIS_PER_SECOND))
    else {
        return private(ctx, text::SOMETHING_WENT_WRONG).await;
    };
    let Some(shifted) = moment.checked_sub(now_unix) else {
        return private(ctx, text::SOMETHING_WENT_WRONG).await;
    };
    ctx.defer_ephemeral().await?;
    store.set_rehearsal_clock(place.guild, shifted).await?;
    let report = signups.tick_in(place.guild, moment, moment_ms).await;
    if report.failures > 0 {
        tracing::warn!(
            guild = place.guild.get(),
            failures = report.failures,
            "a rehearsal step had beat steps fail"
        );
    }
    private(ctx, text::stepped(moment, &report)).await
}

#[poise::command(slash_command, rename = "reset")]
async fn rehearse_reset(ctx: Context<'_>) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    if !rehearsing(ctx, place.guild) {
        return private(ctx, text::REHEARSAL_ONLY).await;
    }
    ctx.defer_ephemeral().await?;
    let purged = ctx
        .data()
        .signups
        .purge(place.guild, super::now_unix())
        .await;
    if purged.failures > 0 {
        tracing::warn!(
            guild = place.guild.get(),
            failures = purged.failures,
            "a rehearsal reset left sign-up posts behind, so nothing was deleted"
        );
        return private(ctx, text::reset_blocked(&purged)).await;
    }
    private(ctx, text::reset_done(&purged)).await
}

#[poise::command(
    slash_command,
    guild_only,
    default_member_permissions = "MANAGE_CHANNELS",
    required_permissions = "MANAGE_CHANNELS"
)]
async fn voice(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
async fn hub(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

fn name_problem(problem: NameProblem) -> String {
    match problem {
        NameProblem::Empty => voice_text::NAME_EMPTY.to_owned(),
        NameProblem::TooLong { limit } => voice_text::name_too_long(limit),
    }
}

fn cleaned(raw: Option<&str>, limit: usize) -> Result<Option<String>, String> {
    raw.map(|raw| clean_name(raw, limit))
        .transpose()
        .map_err(name_problem)
}

async fn own_hub(ctx: Context<'_>, channel: &serenity::GuildChannel) -> Result<Option<Hub>, Error> {
    let Some(guild) = ctx.guild_id() else {
        return Ok(None);
    };
    let found = ctx
        .data()
        .voice
        .store()
        .hub(ChannelId::new(channel.id.get()))
        .await?;
    Ok(found.filter(|hub| hub.guild == GuildId::new(guild.get())))
}

enum Placement {
    Into(serenity::ChannelId),
    TopLevel,
}

#[poise::command(slash_command, rename = "create")]
async fn hub_create(
    ctx: Context<'_>,
    #[description = "Rooms are named this plus a number: cb gives cb-1, cb-2"]
    #[max_length = 90]
    room_name: String,
    #[description = "Name of the Join to Create channel; Join to Create if left empty"]
    #[max_length = 100]
    name: Option<String>,
    #[description = "Category to put it in; none if left empty"]
    #[channel_types("Category")]
    category: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    let Some(guild) = ctx.guild_id() else {
        return Ok(());
    };
    let room_name = match clean_name(&room_name, ROOM_NAME_LIMIT) {
        Ok(room_name) => room_name,
        Err(problem) => return private(ctx, name_problem(problem)).await,
    };
    let name = match clean_name(
        name.as_deref().unwrap_or(voice_text::HUB_DEFAULT_NAME),
        HUB_NAME_LIMIT,
    ) {
        Ok(name) => name,
        Err(problem) => return private(ctx, name_problem(problem)).await,
    };
    let Some(created_at_ms) = super::now_ms() else {
        return private(ctx, text::SOMETHING_WENT_WRONG).await;
    };
    ctx.defer_ephemeral().await?;
    let channel = serenity::CreateChannel::new(name).kind(serenity::ChannelType::Voice);
    let channel = category
        .iter()
        .fold(channel, |channel, category| channel.category(category.id));
    let created = match guild.create_channel(ctx.http(), channel).await {
        Ok(created) => created,
        Err(error) if refused(&error, MISSING_PERMISSIONS) => {
            return private(ctx, voice_text::NEEDS_MANAGE_CHANNELS).await;
        }
        Err(error) => return Err(error.into()),
    };
    let hub = Hub {
        channel: ChannelId::new(created.id.get()),
        guild: GuildId::new(guild.get()),
        room_name,
        created_by: UserId::new(ctx.author().id.get()),
        created_at_ms,
    };
    if let Err(error) = ctx.data().voice.store().add_hub(&hub).await {
        if let Err(cleanup) = created.id.delete(ctx.http()).await {
            tracing::error!(%cleanup, "an unrecorded Join to Create channel could not be removed");
        }
        return Err(error.into());
    }
    private(ctx, voice_text::hub_created(hub.channel, &hub.room_name)).await
}

#[poise::command(slash_command, rename = "edit")]
async fn hub_edit(
    ctx: Context<'_>,
    #[description = "The Join to Create channel to change"]
    #[channel_types("Voice")]
    hub: serenity::GuildChannel,
    #[description = "New name for the Join to Create channel"]
    #[max_length = 100]
    name: Option<String>,
    #[description = "New name for the rooms it opens from now on"]
    #[max_length = 90]
    room_name: Option<String>,
    #[description = "Move it into this category"]
    #[channel_types("Category")]
    category: Option<serenity::GuildChannel>,
    #[description = "Move it out of its category"] top_level: Option<bool>,
) -> Result<(), Error> {
    let Some(stored) = own_hub(ctx, &hub).await? else {
        return private(ctx, voice_text::NOT_A_HUB).await;
    };
    if category.is_some() && top_level.is_some() {
        return private(ctx, voice_text::CATEGORY_AND_TOP_LEVEL).await;
    }
    let name = match cleaned(name.as_deref(), HUB_NAME_LIMIT) {
        Ok(name) => name.filter(|name| *name != hub.name),
        Err(problem) => return private(ctx, problem).await,
    };
    let room_name = match cleaned(room_name.as_deref(), ROOM_NAME_LIMIT) {
        Ok(room_name) => room_name.filter(|room_name| *room_name != stored.room_name),
        Err(problem) => return private(ctx, problem).await,
    };
    let placement = match (category, top_level) {
        (Some(category), _) if hub.parent_id != Some(category.id) => {
            Some(Placement::Into(category.id))
        }
        (_, Some(true)) if hub.parent_id.is_some() => Some(Placement::TopLevel),
        _ => None,
    };
    if name.is_none() && room_name.is_none() && placement.is_none() {
        return private(ctx, voice_text::NOTHING_TO_CHANGE).await;
    }
    ctx.defer_ephemeral().await?;
    if name.is_some() || placement.is_some() {
        let edit = name
            .iter()
            .fold(serenity::EditChannel::new(), |edit, name| edit.name(name));
        let edit = match placement {
            Some(Placement::Into(category)) => edit.category(category),
            Some(Placement::TopLevel) => edit.category(None::<serenity::ChannelId>),
            None => edit,
        };
        match hub.id.edit(ctx.http(), edit).await {
            Ok(_) => {}
            Err(error) if refused(&error, MISSING_PERMISSIONS) => {
                return private(ctx, voice_text::NEEDS_MANAGE_CHANNELS).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
    if let Some(room_name) = &room_name
        && !ctx
            .data()
            .voice
            .store()
            .rename_rooms(stored.channel, room_name)
            .await?
    {
        return private(ctx, voice_text::NOT_A_HUB).await;
    }
    let changes: Vec<String> = [
        name.as_deref().map(voice_text::renamed),
        room_name.as_deref().map(voice_text::rooms_renamed),
        placement.map(|placement| match placement {
            Placement::Into(category) => voice_text::moved_to(ChannelId::new(category.get())),
            Placement::TopLevel => voice_text::MOVED_TO_TOP.to_owned(),
        }),
    ]
    .into_iter()
    .flatten()
    .collect();
    private(ctx, voice_text::hub_edited(stored.channel, &changes)).await
}

#[poise::command(slash_command, rename = "remove")]
async fn hub_remove(
    ctx: Context<'_>,
    #[description = "The Join to Create channel to delete"]
    #[channel_types("Voice")]
    hub: serenity::GuildChannel,
) -> Result<(), Error> {
    let Some(stored) = own_hub(ctx, &hub).await? else {
        return private(ctx, voice_text::NOT_A_HUB).await;
    };
    ctx.defer_ephemeral().await?;
    match hub.id.delete(ctx.http()).await {
        Ok(_) => {}
        Err(error) if refused(&error, UNKNOWN_CHANNEL) => {}
        Err(error) if refused(&error, MISSING_PERMISSIONS) => {
            return private(ctx, voice_text::NEEDS_MANAGE_CHANNELS).await;
        }
        Err(error) => return Err(error.into()),
    }
    ctx.data().voice.store().remove_hub(stored.channel).await?;
    private(ctx, voice_text::hub_removed(&hub.name)).await
}

#[poise::command(slash_command, rename = "list")]
async fn hub_list(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild) = ctx.guild_id() else {
        return Ok(());
    };
    let store = ctx.data().voice.store();
    let hubs = store.hubs_in(GuildId::new(guild.get())).await?;
    if hubs.is_empty() {
        return private(ctx, voice_text::NO_HUBS).await;
    }
    let mut lines = Vec::with_capacity(hubs.len());
    for hub in &hubs {
        let open = store.open_rooms_of(hub.channel).await?;
        lines.push(voice_text::hub_line(hub.channel, &hub.room_name, open));
    }
    private(ctx, voice_text::hub_list(&lines)).await
}

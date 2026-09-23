use std::collections::BTreeMap;
use std::num::NonZeroU64;
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
use crate::attendance::RepostOutcome;
use crate::attendance::RepostPing;
use crate::attendance::next_moment;
use crate::attendance::next_post_at;
use crate::attendance_store::Attendance;
use crate::attendance_store::CreateOutcome;
use crate::attendance_store::EditOutcome;
use crate::attendance_store::EndOutcome;
use crate::attendance_store::NewSeason;
use crate::attendance_store::Post;
use crate::attendance_store::Season;
use crate::attendance_store::SeasonChange;
use crate::failure;
use crate::failure::Failure;
use crate::failure::Scope;
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Ping;
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
use crate::voice::ROOM_ACCESS;
use crate::voice::ROOM_NAME_LIMIT;
use crate::voice::clean_name;
use crate::voice_store::Hub;
use crate::voice_text;
use crate::wiring;
use crate::wiring::ChannelAccess;
use crate::wiring::LeaderboardRequest;
use crate::wiring::PingChoiceProblem;
use crate::wiring::PingReach;
use crate::wiring::PingVerdict;
use crate::wiring::SetupPing;
use crate::wiring::SortChoice;

const SILHOUETTE_FILE: &str = "silhouette.png";
const UNKNOWN_MEMBER: isize = 10007;
const MILLIS_PER_SECOND: u64 = 1000;
const REHEARSAL_NIGHTS: u32 = 3;
const REHEARSAL_NIGHTS_MAX: u32 = 7;
const CODENAME_LIMIT: usize = 100;
const CLOCK_BEFORE_1970: &str = "the clock reads before 1970";
const CLOCK_OUT_OF_RANGE: &str = "the clock reads a time outside the supported calendar";
const SIGNUP_ACCESS: serenity::Permissions = serenity::Permissions::VIEW_CHANNEL
    .union(serenity::Permissions::SEND_MESSAGES)
    .union(serenity::Permissions::EMBED_LINKS)
    .union(serenity::Permissions::READ_MESSAGE_HISTORY);
const MENTION_GRANT: serenity::Permissions =
    serenity::Permissions::MENTION_EVERYONE.union(serenity::Permissions::ADMINISTRATOR);

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
                            "Change a season's number, dates, codename or ping roles".into(),
                        ),
                        ..season_edit()
                    },
                    poise::Command {
                        description: Some(
                            "Send a season's open sign-up post again at the bottom of its channel"
                                .into(),
                        ),
                        ..season_repost()
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

async fn refuse(ctx: Context<'_>, refusal: text::Refusal) -> Result<(), Error> {
    failure::refused(
        "command.refused",
        "a command was refused",
        refusal.code(),
        &Scope::of_command(ctx),
    );
    private(ctx, refusal.message()).await
}

fn reported(ctx: Context<'_>, failure: &Failure) -> String {
    failure::report(
        "command.failed",
        "a command did not finish",
        failure,
        &Scope::of_command(ctx),
    );
    failure::command_failed(&ctx.command().qualified_name, failure)
}

async fn fail(ctx: Context<'_>, failure: Failure) -> Result<(), Error> {
    let reply = reported(ctx, &failure);
    private(ctx, reply).await
}

fn missing_here(names: Vec<&'static str>) -> Option<text::Refusal> {
    (!names.is_empty()).then_some(text::Refusal::MissingBotPermissions {
        names,
        channel: None,
    })
}

fn missing_in_season_channel(ctx: Context<'_>, season: &Season) -> Option<text::Refusal> {
    let granted = failure::barnacle_permissions_in(ctx.cache(), season.guild, season.channel)?;
    let names = (SIGNUP_ACCESS - granted).get_permission_names();
    (!names.is_empty()).then_some(text::Refusal::MissingBotPermissions {
        names,
        channel: Some(season.channel),
    })
}

fn manage_channels_denied(
    error: &serenity::Error,
    granted: &[Option<serenity::Permissions>],
) -> Failure {
    let missing = granted
        .iter()
        .flatten()
        .fold(serenity::Permissions::empty(), |missing, granted| {
            missing | (serenity::Permissions::MANAGE_CHANNELS - *granted)
        });
    if missing.is_empty() {
        Failure::from_error(error)
    } else {
        Failure::missing_permissions(missing.get_permission_names())
    }
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

fn app_permissions(ctx: Context<'_>) -> Option<serenity::Permissions> {
    match ctx {
        poise::Context::Application(app) => app.interaction.app_permissions,
        poise::Context::Prefix(_) => None,
    }
}

fn grants_mention(granted: serenity::Permissions) -> bool {
    granted.intersects(MENTION_GRANT)
}

fn person_permissions_in(ctx: Context<'_>, season: &Season) -> Option<serenity::Permissions> {
    let poise::Context::Application(app) = ctx else {
        return None;
    };
    let member = app.interaction.member.as_deref()?;
    if ctx.channel_id().get() == season.channel.get() {
        return member.permissions;
    }
    let guild = ctx.guild()?;
    let channel = guild
        .channels
        .get(&serenity::ChannelId::from(NonZeroU64::new(
            season.channel.get(),
        )?))?;
    Some(guild.user_permissions_in(channel, member))
}

fn role_mentionable(ctx: Context<'_>, ping: Ping) -> Option<bool> {
    match ping {
        Ping::Everyone => Some(false),
        Ping::Role(role) => {
            let role = serenity::RoleId::from(NonZeroU64::new(role.get())?);
            ctx.guild()?.roles.get(&role).map(|role| role.mentionable)
        }
    }
}

fn ping_reach(
    ctx: Context<'_>,
    ping: Ping,
    guild: GuildId,
    channel_granted: Option<serenity::Permissions>,
) -> Option<PingReach> {
    Some(PingReach {
        role_mentionable: role_mentionable(ctx, ping)?,
        server_grant: grants_mention(failure::barnacle_permissions(ctx.cache(), guild)?),
        channel_grant: grants_mention(channel_granted?),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CheckedPing {
    ping: Ping,
    verdict: PingVerdict,
    setup: SetupPing,
    reach: Option<PingReach>,
    place: Place,
}

fn judge_ping(
    ctx: Context<'_>,
    ping: Ping,
    place: Place,
    channel_granted: Option<serenity::Permissions>,
    creating: bool,
) -> CheckedPing {
    let reach = ping_reach(ctx, ping, place.guild, channel_granted);
    let verdict = wiring::ping_verdict(ping, reach);
    CheckedPing {
        ping,
        verdict,
        setup: wiring::setup_ping(creating, verdict),
        reach,
        place,
    }
}

fn judge_pings(
    ctx: Context<'_>,
    pings: &[Ping],
    place: Place,
    channel_granted: Option<serenity::Permissions>,
    creating: bool,
) -> Vec<CheckedPing> {
    pings
        .iter()
        .map(|ping| judge_ping(ctx, *ping, place, channel_granted, creating))
        .collect()
}

fn log_ping_checks(ctx: Context<'_>, checked: &[CheckedPing], refused: bool) {
    for checked in checked {
        failure::ping_checked(
            checked.ping,
            checked.verdict,
            checked.reach,
            refused,
            &Scope::of_command(ctx)
                .guild(checked.place.guild)
                .channel(checked.place.channel),
        );
    }
}

fn check_pings(
    ctx: Context<'_>,
    pings: &[Ping],
    place: Place,
    channel_granted: Option<serenity::Permissions>,
    creating: bool,
) -> Vec<CheckedPing> {
    let checked = judge_pings(ctx, pings, place, channel_granted, creating);
    log_ping_checks(ctx, &checked, false);
    checked
}

fn chosen_ping_roles(
    guild: GuildId,
    given: [Option<serenity::RoleId>; 5],
) -> Result<Vec<RoleId>, text::Refusal> {
    wiring::ping_choice(
        guild,
        given.map(|role| role.map(|role| RoleId::new(role.get()))),
    )
    .map_err(|problem| match problem {
        PingChoiceProblem::MixedEveryone => text::Refusal::PingMixesEveryone,
    })
}

fn pings_of(guild: GuildId, roles: &[RoleId]) -> Vec<Ping> {
    roles.iter().map(|role| Ping::of(guild, *role)).collect()
}

fn season_pings(season: &Season) -> Vec<Ping> {
    pings_of(season.guild, &season.ping_roles)
}

fn ping_warning(checked: &[CheckedPing], channel: ChannelId) -> Option<String> {
    let verdicts: Vec<(Ping, PingVerdict)> = checked
        .iter()
        .map(|checked| (checked.ping, checked.verdict))
        .collect();
    text::silent_ping_warning(&verdicts, channel)
}

fn warned(warning: Option<String>, reply: String) -> String {
    match warning {
        Some(warning) => format!("{warning}\n\n{reply}"),
        None => reply,
    }
}

async fn refuse_blocked_ping(
    ctx: Context<'_>,
    checked: &[CheckedPing],
    channel: ChannelId,
) -> Result<bool, Error> {
    let pings: Vec<Ping> = checked
        .iter()
        .filter(|checked| checked.setup == SetupPing::Refuse)
        .map(|checked| checked.ping)
        .collect();
    if pings.is_empty() {
        return Ok(false);
    }
    log_ping_checks(ctx, checked, true);
    refuse(ctx, text::Refusal::PingBlockedByChannel { pings, channel }).await?;
    Ok(true)
}

async fn remove_round_post(ctx: Context<'_>) {
    if let poise::Context::Application(app) = ctx
        && let Err(error) = app.interaction.delete_response(ctx.http()).await
    {
        failure::report(
            "command.failed",
            "a round post that timed out could not be removed",
            &Failure::from_error(&error),
            &Scope::of_command(ctx),
        );
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
    if let Some(refusal) = missing_here(wiring::missing_permissions(channel_access(ctx))) {
        return refuse(ctx, refusal).await;
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
                        failure::report(
                            "command.failed",
                            "an untracked round post could not be removed",
                            &Failure::from_error(&cleanup),
                            &Scope::of_command(ctx),
                        );
                    }
                    Err(error.into())
                }
            }
        })
        .await;
    match outcome {
        StartOutcome::Started { .. } => Ok(()),
        StartOutcome::Busy => refuse(ctx, text::Refusal::RoundAlreadyRunning).await,
        StartOutcome::NoShips(_) => refuse(ctx, text::Refusal::EmptyPool(options)).await,
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
        return refuse(ctx, text::Refusal::NoShipMatches).await;
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
        Err(error) => serenity::EditInteractionResponse::new()
            .content(reported(ctx, &Failure::from_error(&*error))),
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
#[allow(clippy::too_many_arguments)]
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
    #[description = "Another role to ping when a sign-up is posted"] ping_role_2: Option<
        serenity::RoleId,
    >,
    #[description = "Another role to ping when a sign-up is posted"] ping_role_3: Option<
        serenity::RoleId,
    >,
    #[description = "Another role to ping when a sign-up is posted"] ping_role_4: Option<
        serenity::RoleId,
    >,
    #[description = "Another role to ping when a sign-up is posted"] ping_role_5: Option<
        serenity::RoleId,
    >,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    if channel_kind(ctx) != Some(serenity::ChannelType::Text) {
        return refuse(ctx, text::Refusal::WrongChannelType).await;
    }
    if let Some(refusal) = missing_here(wiring::missing_signup_permissions(channel_access(ctx))) {
        return refuse(ctx, refusal).await;
    }
    let (Some(first), Some(last)) = (parse_day(&first_day), parse_day(&last_day)) else {
        return refuse(ctx, text::Refusal::DateFormat).await;
    };
    let Some(range) = Range::new(first, last) else {
        return refuse(ctx, text::Refusal::LastDayBeforeFirst).await;
    };
    let now_unix = super::now_unix();
    let Some(today) = schedule::today(now_unix) else {
        return fail(ctx, Failure::internal(CLOCK_OUT_OF_RANGE)).await;
    };
    if !range.near(today) {
        return refuse(ctx, text::Refusal::DatesTooFar).await;
    }
    let nights_left = range.nights_left(now_unix);
    if nights_left == 0 {
        return refuse(ctx, text::Refusal::NoNightsLeft).await;
    }
    let Some(created_at_ms) = super::now_ms() else {
        return fail(ctx, Failure::internal(CLOCK_BEFORE_1970)).await;
    };
    let ping_roles = match chosen_ping_roles(
        place.guild,
        [
            ping_role,
            ping_role_2,
            ping_role_3,
            ping_role_4,
            ping_role_5,
        ],
    ) {
        Ok(roles) => roles,
        Err(refusal) => return refuse(ctx, refusal).await,
    };
    let checked = judge_pings(
        ctx,
        &pings_of(place.guild, &ping_roles),
        place,
        app_permissions(ctx),
        true,
    );
    if refuse_blocked_ping(ctx, &checked, place.channel).await? {
        return Ok(());
    }
    let new = NewSeason {
        guild: place.guild,
        channel: place.channel,
        number,
        codename: clipped_codename(codename),
        range,
        created_by: UserId::new(ctx.author().id.get()),
        created_at_ms,
        ping_roles,
    };
    match ctx.data().signups.store().create_season(&new).await? {
        CreateOutcome::Created(season) => {
            log_ping_checks(ctx, &checked, false);
            let started = text::season_started(&season, nights_left, next_post_at(range, now_unix));
            private(ctx, warned(ping_warning(&checked, place.channel), started)).await
        }
        CreateOutcome::NumberTaken => {
            log_ping_checks(ctx, &checked, true);
            refuse(ctx, text::Refusal::SeasonNumberTaken(number)).await
        }
        CreateOutcome::Overlaps(other) => {
            log_ping_checks(ctx, &checked, true);
            refuse(ctx, text::Refusal::SeasonOverlaps(other)).await
        }
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
    #[description = "Another role to ping when a sign-up is posted"] ping_role_2: Option<
        serenity::RoleId,
    >,
    #[description = "Another role to ping when a sign-up is posted"] ping_role_3: Option<
        serenity::RoleId,
    >,
    #[description = "Another role to ping when a sign-up is posted"] ping_role_4: Option<
        serenity::RoleId,
    >,
    #[description = "Another role to ping when a sign-up is posted"] ping_role_5: Option<
        serenity::RoleId,
    >,
    #[description = "Remove the codename"] clear_codename: Option<bool>,
    #[description = "Stop pinging every role"] clear_ping_role: Option<bool>,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    let clearing_codename = clear_codename.unwrap_or(false);
    let clearing_ping_role = clear_ping_role.unwrap_or(false);
    let given_ping_roles = [
        ping_role,
        ping_role_2,
        ping_role_3,
        ping_role_4,
        ping_role_5,
    ];
    let naming_ping_roles = given_ping_roles.iter().any(Option::is_some);
    let named = new_number.is_some()
        || first_day.is_some()
        || last_day.is_some()
        || codename.is_some()
        || naming_ping_roles
        || clearing_codename
        || clearing_ping_role;
    if !named {
        return refuse(ctx, text::Refusal::NothingToChange).await;
    }
    if codename.is_some() && clearing_codename {
        return refuse(ctx, text::Refusal::CodenameBothWays).await;
    }
    if naming_ping_roles && clearing_ping_role {
        return refuse(ctx, text::Refusal::PingBothWays).await;
    }
    let chosen_roles = match chosen_ping_roles(place.guild, given_ping_roles) {
        Ok(roles) => roles,
        Err(refusal) => return refuse(ctx, refusal).await,
    };
    let first = first_day.as_deref().map(parse_day);
    let last = last_day.as_deref().map(parse_day);
    if matches!(first, Some(None)) || matches!(last, Some(None)) {
        return refuse(ctx, text::Refusal::DateFormat).await;
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
        ping_roles: if clearing_ping_role {
            Some(Vec::new())
        } else {
            naming_ping_roles.then_some(chosen_roles)
        },
    };
    let now_unix = super::now_unix();
    let Some(today) = schedule::today(now_unix) else {
        return fail(ctx, Failure::internal(CLOCK_OUT_OF_RANGE)).await;
    };
    let signups = &ctx.data().signups;
    let Some(season) = signups.store().live_season(place.guild, number).await? else {
        return refuse(ctx, text::Refusal::SeasonNotFound(number)).await;
    };
    if let Some(refusal) = missing_in_season_channel(ctx, &season) {
        return refuse(ctx, refusal).await;
    }
    let outcome = signups
        .store()
        .edit_season(place.guild, number, &change, today)
        .await?;
    match outcome {
        EditOutcome::Edited { after, .. } => {
            ctx.defer_ephemeral().await?;
            let cleared = signups
                .clear_posts(&after, ClearScope::OutsideRange, now_unix)
                .await;
            let refreshed = signups.refresh_posts(&after, now_unix).await;
            let edited = text::season_edited(
                &after,
                after.range.nights_left(now_unix),
                next_post_at(after.range, now_unix),
                cleared.touched,
            );
            let posts_in = Place {
                guild: after.guild,
                channel: after.channel,
            };
            let checked = check_pings(
                ctx,
                &season_pings(&after),
                posts_in,
                failure::barnacle_permissions_in(ctx.cache(), after.guild, after.channel),
                false,
            );
            private(
                ctx,
                warned(
                    ping_warning(&checked, after.channel),
                    text::season_edit_reached(&edited, &cleared, &refreshed),
                ),
            )
            .await
        }
        EditOutcome::NumberTaken => {
            refuse(
                ctx,
                text::Refusal::SeasonNumberTaken(new_number.unwrap_or(number)),
            )
            .await
        }
        EditOutcome::Overlaps(other) => refuse(ctx, text::Refusal::SeasonOverlaps(other)).await,
        EditOutcome::BadRange => refuse(ctx, text::Refusal::LastDayBeforeFirst).await,
        EditOutcome::TooFar => refuse(ctx, text::Refusal::DatesTooFar).await,
        EditOutcome::NotFound => refuse(ctx, text::Refusal::SeasonNotFound(number)).await,
    }
}

#[poise::command(slash_command, rename = "repost")]
async fn season_repost(
    ctx: Context<'_>,
    #[description = "Which season to repost, for example 35"]
    #[min = 1]
    number: u32,
    #[description = "Ping the season's roles again"] ping_again: Option<bool>,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    let pinging_again = ping_again.unwrap_or(false);
    let signups = &ctx.data().signups;
    let Some(season) = signups.store().live_season(place.guild, number).await? else {
        return refuse(ctx, text::Refusal::SeasonNotFound(number)).await;
    };
    if let Some(refusal) = missing_in_season_channel(ctx, &season) {
        return refuse(ctx, refusal).await;
    }
    let pings = season_pings(&season);
    let checked = if pinging_again {
        if pings.is_empty() {
            return refuse(ctx, text::Refusal::RepostNothingToPing).await;
        }
        let mentionable: Vec<bool> = pings
            .iter()
            .map(|ping| role_mentionable(ctx, *ping).unwrap_or(false))
            .collect();
        let person = person_permissions_in(ctx, &season).map(grants_mention);
        if !wiring::may_ping_again(&pings, person, &mentionable) {
            return refuse(ctx, text::Refusal::RepostPingNotAllowed).await;
        }
        judge_pings(
            ctx,
            &pings,
            Place {
                guild: season.guild,
                channel: season.channel,
            },
            failure::barnacle_permissions_in(ctx.cache(), season.guild, season.channel),
            false,
        )
    } else {
        Vec::new()
    };
    let Some(now_ms) = super::now_ms() else {
        return fail(ctx, Failure::internal(CLOCK_BEFORE_1970)).await;
    };
    let now_unix = super::now_unix();
    let ping = if pinging_again {
        RepostPing::Again {
            checked: season.ping_roles.clone(),
            channel: season.channel,
        }
    } else {
        RepostPing::Quiet
    };
    ctx.defer_ephemeral().await?;
    match signups
        .repost(place.guild, number, ping, now_unix, now_ms)
        .await
    {
        RepostOutcome::Reposted {
            night,
            adopted,
            pinged,
            ..
        } => {
            log_ping_checks(ctx, &checked, false);
            private(
                ctx,
                warned(
                    ping_warning(&checked, season.channel),
                    text::reposted(&season, night, adopted, &pinged),
                ),
            )
            .await
        }
        RepostOutcome::NotFound => {
            log_ping_checks(ctx, &checked, true);
            refuse(ctx, text::Refusal::SeasonNotFound(number)).await
        }
        RepostOutcome::NothingOpen {
            next_post_at,
            nights_left,
        } => {
            log_ping_checks(ctx, &checked, true);
            refuse(
                ctx,
                text::Refusal::NothingToRepost {
                    number,
                    next_post_at,
                    nights_left,
                },
            )
            .await
        }
        RepostOutcome::PingsChanged => {
            log_ping_checks(ctx, &checked, true);
            refuse(ctx, text::Refusal::RepostPingsChanged).await
        }
        RepostOutcome::Superseded => {
            log_ping_checks(ctx, &checked, true);
            refuse(ctx, text::Refusal::RepostSuperseded).await
        }
        RepostOutcome::Failed(failure) => fail(ctx, failure).await,
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
        return refuse(ctx, text::Refusal::WrongChannelType).await;
    }
    if let Some(refusal) = missing_here(wiring::missing_signup_permissions(channel_access(ctx))) {
        return refuse(ctx, refusal).await;
    }
    let signups = &ctx.data().signups;
    let Some(season) = signups.store().live_season(place.guild, number).await? else {
        return refuse(ctx, text::Refusal::SeasonNotFound(number)).await;
    };
    if season.channel == place.channel {
        return refuse(ctx, text::Refusal::SeasonAlreadyHere(number)).await;
    }
    if let Some(refusal) = missing_in_season_channel(ctx, &season) {
        return refuse(ctx, refusal).await;
    }
    let now_unix = super::now_unix();
    ctx.defer_ephemeral().await?;
    let cleared = signups
        .clear_posts(&season, ClearScope::All, now_unix)
        .await;
    if cleared.failures > 0 {
        return private(
            ctx,
            text::failures_noted(text::CLEAR_FAILED_MOVE, cleared.failures),
        )
        .await;
    }
    if !signups
        .store()
        .move_season(place.guild, season.id, place.channel)
        .await?
    {
        return refuse(ctx, text::Refusal::SeasonNotFound(number)).await;
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
    let checked = check_pings(
        ctx,
        &season_pings(&after),
        place,
        app_permissions(ctx),
        false,
    );
    private(ctx, warned(ping_warning(&checked, place.channel), moved)).await
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
        return fail(ctx, Failure::internal(CLOCK_BEFORE_1970)).await;
    };
    let now_unix = super::now_unix();
    let signups = &ctx.data().signups;
    let Some(season) = signups.store().live_season(place.guild, number).await? else {
        return refuse(ctx, text::Refusal::SeasonNotFound(number)).await;
    };
    if let Some(refusal) = missing_in_season_channel(ctx, &season) {
        return refuse(ctx, refusal).await;
    }
    ctx.defer_ephemeral().await?;
    let cleared = signups
        .clear_posts(&season, ClearScope::All, now_unix)
        .await;
    if cleared.failures > 0 {
        return private(
            ctx,
            text::failures_noted(text::CLEAR_FAILED_END, cleared.failures),
        )
        .await;
    }
    match signups
        .store()
        .end_season(place.guild, number, now_ms)
        .await?
    {
        EndOutcome::Removed => private(ctx, text::season_removed(number)).await,
        EndOutcome::Ended { .. } => private(ctx, text::season_ended(number, cleared.touched)).await,
        EndOutcome::NotFound => refuse(ctx, text::Refusal::SeasonNotFound(number)).await,
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
#[allow(clippy::too_many_arguments)]
async fn rehearse_start(
    ctx: Context<'_>,
    #[description = "Which CB season this is, for example 99"]
    #[min = 1]
    number: u32,
    #[description = "The season's codename, for example Komodo Dragon"]
    #[max_length = 100]
    codename: Option<String>,
    #[description = "Role to ping when a sign-up is posted"] ping_role: Option<serenity::RoleId>,
    #[description = "Another role to ping when a sign-up is posted"] ping_role_2: Option<
        serenity::RoleId,
    >,
    #[description = "Another role to ping when a sign-up is posted"] ping_role_3: Option<
        serenity::RoleId,
    >,
    #[description = "Another role to ping when a sign-up is posted"] ping_role_4: Option<
        serenity::RoleId,
    >,
    #[description = "Another role to ping when a sign-up is posted"] ping_role_5: Option<
        serenity::RoleId,
    >,
    #[description = "How many nights to rehearse, one per day from tomorrow (default 3)"]
    #[min = 1]
    #[max = 7]
    nights: Option<u32>,
) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    if !rehearsing(ctx, place.guild) {
        return refuse(ctx, text::Refusal::RehearsalOnly).await;
    }
    if channel_kind(ctx) != Some(serenity::ChannelType::Text) {
        return refuse(ctx, text::Refusal::WrongChannelType).await;
    }
    if let Some(refusal) = missing_here(wiring::missing_signup_permissions(channel_access(ctx))) {
        return refuse(ctx, refusal).await;
    }
    let now_unix = super::now_unix();
    let Some(today) = schedule::today(now_unix) else {
        return fail(ctx, Failure::internal(CLOCK_OUT_OF_RANGE)).await;
    };
    let Some(range) = rehearsal_range(today, nights.unwrap_or(REHEARSAL_NIGHTS), now_unix) else {
        return fail(
            ctx,
            Failure::internal("the rehearsal nights fall outside the supported calendar"),
        )
        .await;
    };
    if !range.near(today) {
        return refuse(ctx, text::Refusal::DatesTooFar).await;
    }
    let nights_left = range.nights_left(now_unix);
    let Some(created_at_ms) = super::now_ms() else {
        return fail(ctx, Failure::internal(CLOCK_BEFORE_1970)).await;
    };
    let ping_roles = match chosen_ping_roles(
        place.guild,
        [
            ping_role,
            ping_role_2,
            ping_role_3,
            ping_role_4,
            ping_role_5,
        ],
    ) {
        Ok(roles) => roles,
        Err(refusal) => return refuse(ctx, refusal).await,
    };
    let checked = judge_pings(
        ctx,
        &pings_of(place.guild, &ping_roles),
        place,
        app_permissions(ctx),
        true,
    );
    if refuse_blocked_ping(ctx, &checked, place.channel).await? {
        return Ok(());
    }
    let new = NewSeason {
        guild: place.guild,
        channel: place.channel,
        number,
        codename: clipped_codename(codename),
        range,
        created_by: UserId::new(ctx.author().id.get()),
        created_at_ms,
        ping_roles,
    };
    match ctx.data().signups.store().create_season(&new).await? {
        CreateOutcome::Created(season) => {
            log_ping_checks(ctx, &checked, false);
            let started =
                text::rehearsal_started(&season, nights_left, next_post_at(range, now_unix));
            private(ctx, warned(ping_warning(&checked, place.channel), started)).await
        }
        CreateOutcome::NumberTaken => {
            log_ping_checks(ctx, &checked, true);
            refuse(ctx, text::Refusal::SeasonNumberTaken(number)).await
        }
        CreateOutcome::Overlaps(other) => {
            log_ping_checks(ctx, &checked, true);
            refuse(ctx, text::Refusal::SeasonOverlaps(other)).await
        }
    }
}

#[poise::command(slash_command, rename = "next")]
async fn rehearse_next(ctx: Context<'_>) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    if !rehearsing(ctx, place.guild) {
        return refuse(ctx, text::Refusal::RehearsalOnly).await;
    }
    let signups = &ctx.data().signups;
    let store = signups.store();
    let now_unix = super::now_unix();
    let offset = store.rehearsal_clock(place.guild).await?;
    let Some(rehearsal_now) = now_unix.checked_add(offset) else {
        return fail(
            ctx,
            Failure::internal("the rehearsal clock offset overflows the current time"),
        )
        .await;
    };
    let seasons = store.seasons_in(place.guild).await?;
    let posts = posts_of(store, &seasons).await?;
    let Some(moment) = next_moment(&seasons, &posts, rehearsal_now) else {
        return refuse(ctx, text::Refusal::NothingPending).await;
    };
    let Some(moment_ms) = u64::try_from(moment)
        .ok()
        .and_then(|seconds| seconds.checked_mul(MILLIS_PER_SECOND))
    else {
        return fail(
            ctx,
            Failure::internal("the next rehearsal step falls outside the clock's range"),
        )
        .await;
    };
    let Some(shifted) = moment.checked_sub(now_unix) else {
        return fail(
            ctx,
            Failure::internal("the rehearsal clock shift overflows"),
        )
        .await;
    };
    ctx.defer_ephemeral().await?;
    store.set_rehearsal_clock(place.guild, shifted).await?;
    let report = signups.tick_in(place.guild, moment, moment_ms).await;
    private(
        ctx,
        text::failures_noted(&text::stepped(moment, &report), report.failures),
    )
    .await
}

#[poise::command(slash_command, rename = "reset")]
async fn rehearse_reset(ctx: Context<'_>) -> Result<(), Error> {
    let Some(place) = place(ctx) else {
        return Ok(());
    };
    if !rehearsing(ctx, place.guild) {
        return refuse(ctx, text::Refusal::RehearsalOnly).await;
    }
    ctx.defer_ephemeral().await?;
    let purged = ctx
        .data()
        .signups
        .purge(place.guild, super::now_unix())
        .await;
    if purged.failures > 0 {
        return private(
            ctx,
            text::failures_noted(&text::reset_blocked(&purged), purged.failures),
        )
        .await;
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

fn name_problem(problem: NameProblem) -> text::Refusal {
    match problem {
        NameProblem::Empty => text::Refusal::NameEmpty,
        NameProblem::TooLong { limit } => text::Refusal::NameTooLong { limit },
    }
}

fn cleaned(raw: Option<&str>, limit: usize) -> Result<Option<String>, text::Refusal> {
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

impl Placement {
    fn category(&self) -> Option<ChannelId> {
        match self {
            Placement::Into(category) => Some(ChannelId::new(category.get())),
            Placement::TopLevel => None,
        }
    }
}

fn room_blockers(
    ctx: Context<'_>,
    guild: GuildId,
    channel: &serenity::GuildChannel,
) -> Vec<&'static str> {
    failure::barnacle_permissions(ctx.cache(), guild)
        .map(|granted| {
            failure::copy_blockers(
                granted,
                &channel.permission_overwrites,
                serenity::Permissions::from_bits_truncate(ROOM_ACCESS),
            )
        })
        .unwrap_or_default()
}

fn cached_channel(
    ctx: Context<'_>,
    guild: GuildId,
    channel: ChannelId,
) -> Option<serenity::GuildChannel> {
    let guild = ctx.cache().guild(NonZeroU64::new(guild.get())?)?;
    guild
        .channels
        .get(&serenity::ChannelId::from(NonZeroU64::new(channel.get())?))
        .cloned()
}

fn cached_room_blockers(ctx: Context<'_>, guild: GuildId, hub: ChannelId) -> Vec<&'static str> {
    cached_channel(ctx, guild, hub)
        .map(|channel| room_blockers(ctx, guild, &channel))
        .unwrap_or_default()
}

fn with_readiness(reply: String, blockers: &[&str]) -> String {
    if blockers.is_empty() {
        reply
    } else {
        format!("{reply} {}", voice_text::cannot_open_rooms(blockers))
    }
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
        Err(problem) => return refuse(ctx, name_problem(problem)).await,
    };
    let name = match clean_name(
        name.as_deref().unwrap_or(voice_text::HUB_DEFAULT_NAME),
        HUB_NAME_LIMIT,
    ) {
        Ok(name) => name,
        Err(problem) => return refuse(ctx, name_problem(problem)).await,
    };
    let Some(created_at_ms) = super::now_ms() else {
        return fail(ctx, Failure::internal(CLOCK_BEFORE_1970)).await;
    };
    ctx.defer_ephemeral().await?;
    let guild_id = GuildId::new(guild.get());
    let channel = serenity::CreateChannel::new(name).kind(serenity::ChannelType::Voice);
    let channel = category
        .iter()
        .fold(channel, |channel, category| channel.category(category.id));
    let created = match guild.create_channel(ctx.http(), channel).await {
        Ok(created) => created,
        Err(error) if refused(&error, MISSING_PERMISSIONS) => {
            let granted = match &category {
                Some(category) => failure::barnacle_permissions_in(
                    ctx.cache(),
                    guild_id,
                    ChannelId::new(category.id.get()),
                ),
                None => failure::barnacle_permissions(ctx.cache(), guild_id),
            };
            return fail(ctx, manage_channels_denied(&error, &[granted])).await;
        }
        Err(error) => return Err(error.into()),
    };
    let hub = Hub {
        channel: ChannelId::new(created.id.get()),
        guild: guild_id,
        room_name,
        created_by: UserId::new(ctx.author().id.get()),
        created_at_ms,
    };
    if let Err(error) = ctx.data().voice.store().add_hub(&hub).await {
        if let Err(cleanup) = created.id.delete(ctx.http()).await {
            failure::report(
                "command.failed",
                "an unrecorded Join to Create channel could not be removed",
                &Failure::from_error(&cleanup),
                &Scope::of_command(ctx).hub(hub.channel),
            );
        }
        return Err(error.into());
    }
    let blockers = room_blockers(ctx, hub.guild, &created);
    private(
        ctx,
        with_readiness(
            voice_text::hub_created(hub.channel, &hub.room_name),
            &blockers,
        ),
    )
    .await
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
        return refuse(ctx, text::Refusal::NotAHub).await;
    };
    if category.is_some() && top_level.is_some() {
        return refuse(ctx, text::Refusal::CategoryAndTopLevel).await;
    }
    let name = match cleaned(name.as_deref(), HUB_NAME_LIMIT) {
        Ok(name) => name.filter(|name| *name != hub.name),
        Err(problem) => return refuse(ctx, problem).await,
    };
    let room_name = match cleaned(room_name.as_deref(), ROOM_NAME_LIMIT) {
        Ok(room_name) => room_name.filter(|room_name| *room_name != stored.room_name),
        Err(problem) => return refuse(ctx, problem).await,
    };
    let placement = match (category, top_level) {
        (Some(category), _) if hub.parent_id != Some(category.id) => {
            Some(Placement::Into(category.id))
        }
        (_, Some(true)) if hub.parent_id.is_some() => Some(Placement::TopLevel),
        _ => None,
    };
    if name.is_none() && room_name.is_none() && placement.is_none() {
        return refuse(ctx, text::Refusal::HubNothingToChange).await;
    }
    ctx.defer_ephemeral().await?;
    let edited = if name.is_some() || placement.is_some() {
        let edit = name
            .iter()
            .fold(serenity::EditChannel::new(), |edit, name| edit.name(name));
        let edit = match placement {
            Some(Placement::Into(category)) => edit.category(category),
            Some(Placement::TopLevel) => edit.category(None::<serenity::ChannelId>),
            None => edit,
        };
        match hub.id.edit(ctx.http(), edit).await {
            Ok(channel) => Some(channel),
            Err(error) if refused(&error, MISSING_PERMISSIONS) => {
                let granted: Vec<Option<serenity::Permissions>> = std::iter::once(stored.channel)
                    .chain(placement.as_ref().and_then(Placement::category))
                    .map(|channel| {
                        failure::barnacle_permissions_in(ctx.cache(), stored.guild, channel)
                    })
                    .collect();
                return fail(ctx, manage_channels_denied(&error, &granted)).await;
            }
            Err(error) => return Err(error.into()),
        }
    } else {
        None
    };
    if let Some(room_name) = &room_name
        && !ctx
            .data()
            .voice
            .store()
            .rename_rooms(stored.channel, room_name)
            .await?
    {
        return refuse(ctx, text::Refusal::NotAHub).await;
    }
    let blockers = match &edited {
        Some(channel) => room_blockers(ctx, stored.guild, channel),
        None => cached_room_blockers(ctx, stored.guild, stored.channel),
    };
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
    private(
        ctx,
        with_readiness(voice_text::hub_edited(stored.channel, &changes), &blockers),
    )
    .await
}

#[poise::command(slash_command, rename = "remove")]
async fn hub_remove(
    ctx: Context<'_>,
    #[description = "The Join to Create channel to delete"]
    #[channel_types("Voice")]
    hub: serenity::GuildChannel,
) -> Result<(), Error> {
    let Some(stored) = own_hub(ctx, &hub).await? else {
        return refuse(ctx, text::Refusal::NotAHub).await;
    };
    ctx.defer_ephemeral().await?;
    match hub.id.delete(ctx.http()).await {
        Ok(_) => {}
        Err(error) if refused(&error, UNKNOWN_CHANNEL) => {}
        Err(error) if refused(&error, MISSING_PERMISSIONS) => {
            let granted =
                failure::barnacle_permissions_in(ctx.cache(), stored.guild, stored.channel);
            return fail(ctx, manage_channels_denied(&error, &[granted])).await;
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
        let blockers = cached_room_blockers(ctx, hub.guild, hub.channel);
        lines.push(voice_text::hub_line_ready(
            hub.channel,
            &hub.room_name,
            open,
            &blockers,
        ));
    }
    private(ctx, voice_text::hub_list(&lines)).await
}

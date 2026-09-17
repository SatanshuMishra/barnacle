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
use crate::ids::ChannelId;
use crate::ids::GuildId;
use crate::ids::Place;
use crate::info;
use crate::solves::Standing;
use crate::table::StartOutcome;
use crate::text;
use crate::wiring;
use crate::wiring::ChannelAccess;
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
    let embeds = pages
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
        .collect();
    app.interaction
        .edit_response(
            ctx.http(),
            serenity::EditInteractionResponse::new().embeds(embeds),
        )
        .await?;
    Ok(())
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

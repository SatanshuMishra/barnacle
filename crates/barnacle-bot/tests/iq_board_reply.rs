use barnacle_bot::discord;
use barnacle_bot::iq;
use barnacle_bot::iq::Standing;
use barnacle_bot::text;
use barnacle_guess::UserId;
use poise::serenity_prelude as serenity;

const EPHEMERAL: u64 = 64;

#[test]
fn the_reply_is_one_private_embed() {
    let reply = serde_json::to_value(
        discord::board_reply("text".to_owned())
            .to_slash_initial_response(serenity::CreateInteractionResponseMessage::new()),
    )
    .unwrap();
    assert_eq!(reply["flags"], EPHEMERAL, "{reply}");
    assert!(reply["content"].is_null(), "{reply}");
    let embeds = reply["embeds"].as_array().cloned().unwrap_or_default();
    assert_eq!(
        embeds,
        [serde_json::to_value(
            serenity::CreateEmbed::new()
                .colour(text::EMBED_COLOUR)
                .description("text")
        )
        .unwrap()],
        "{reply}"
    );
    assert_eq!(
        reply["allowed_mentions"],
        serde_json::to_value(serenity::CreateAllowedMentions::new()).unwrap(),
        "{reply}"
    );
}

#[test]
fn top_rows_highlight_nobody() {
    let standings: Vec<Standing> = (1..=12_u64)
        .map(|place| Standing {
            rank: usize::try_from(place).unwrap(),
            user: UserId::new(10 + place),
            points: 100 - i64::try_from(place).unwrap(),
        })
        .collect();
    let rows = iq::top_rows(&standings);
    assert_eq!(rows.len(), 10, "{rows:?}");
    for (row, standing) in rows.iter().zip(&standings) {
        assert_eq!(row.rank, Some(standing.rank), "{row:?}");
        assert_eq!(row.user, standing.user, "{row:?}");
        assert_eq!(row.points, standing.points, "{row:?}");
        assert!(!row.requester, "{row:?}");
    }
}

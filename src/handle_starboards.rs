use crate::data::AppState;
use color_eyre::eyre::{bail, eyre, Result};
use poise::serenity_prelude::{self as serenity};
use serenity::{Message, Reaction, ReactionType};

pub async fn handle_starboards(
    ctx: &serenity::Context,
    data: &AppState,
    message: &Message,
    reaction: &Reaction,
) -> Result<()> {
    let reaction_type = &reaction.emoji;

    let name = match reaction_type {
        ReactionType::Unicode(string) => emojis::get(string)
            .ok_or_else(|| eyre!("Default emojis should always be in unicode {string}"))?
            .name()
            .to_owned(),
        ReactionType::Custom { id, .. } => id.to_string(),
        _ => bail!("Unknown reaction type"),
    };

    let reaction_count = message
        .reactions
        .iter()
        .find(|reaction| reaction.reaction_type == *reaction_type)
        .map_or(0, |reaction| reaction.count);

    let config = data.config.read().await;

    let futures = config.starboards.iter().map(|starboard| async {
        if starboard
            .does_starboard_apply(ctx, message, reaction_count, &name)
            .await
        {
            starboard.reply(ctx, message, reaction_type).await.ok();
        }
    });

    futures::future::join_all(futures).await;

    Ok(())
}

use crate::data::State;
use color_eyre::eyre::Result;
use poise::serenity_prelude::{Context, Message, Reaction};

pub async fn handle_starboards(
    ctx: &Context,
    data: State,
    message: &Message,
    reaction: &Reaction,
) -> Result<()> {
    let reaction_type = &reaction.emoji;

    let reaction_count = message
        .reactions
        .iter()
        .find(|reaction| reaction.reaction_type == *reaction_type)
        .map_or(0, |reaction| reaction.count);

    let config = data.config.read().await;

    let futures = config.starboards.iter().map(|starboard| async {
        if starboard
            .does_starboard_apply(ctx, message, reaction_count)
            .await
        {
            starboard.reply(ctx, message, reaction_type).await.ok();
        }
    });

    futures::future::join_all(futures).await;

    Ok(())
}

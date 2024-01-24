use std::sync::Arc;

use crate::data::Data;

use poise::serenity_prelude::{self as serenity, Channel, ChannelType};
use serenity::{Message, Reaction, ReactionType};

pub async fn reaction_management(
    ctx: &serenity::Context,
    data: &Data,
    reaction: &Reaction,
) -> anyhow::Result<()> {
    let message = reaction.message(ctx).await?;
    starboard(ctx, data, &message, reaction).await
}

pub async fn starboard(
    ctx: &serenity::Context,
    data: &Data,
    message: &Message,
    reaction: &Reaction,
) -> anyhow::Result<()> {
    let reaction_type = &reaction.emoji;

    let name = match reaction_type {
        ReactionType::Unicode(string) => emojis::get(string)
            .expect("Default emojis should always be in unicode")
            .name()
            .to_owned(),
        ReactionType::Custom { id, .. } => id.to_string(),
        _ => anyhow::bail!("Unknown reaction type"),
    };

    // TODO add this feature to starboard (it's completely not hooked up right now.)
    let _is_forum = message
        .channel_id
        .to_channel(ctx)
        .await
        .map(|channel| match channel {
            Channel::Guild(channel) => channel.kind == ChannelType::Forum,
            _ => false,
        });

    let reaction_count = message
        .reactions
        .iter()
        .find(|reaction| reaction.reaction_type == *reaction_type)
        .map_or(0, |reaction| reaction.count);

    let config = data.config.read().await;

    let message_link = Arc::new(message.link());

    let futures = config.starboards.iter().map(|starboard| async {
        if starboard.does_starboard_apply(reaction_count, &name).await {
            let has_reply = starboard
                .does_channel_already_have_reply(ctx, &message_link)
                .await;

            if !has_reply.unwrap_or(true) {
                starboard
                    .generate_reply(ctx, message, reaction_type)
                    .await
                    .ok();
            }
        }
    });

    futures::future::join_all(futures).await;

    Ok(())
}

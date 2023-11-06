use std::sync::RwLockReadGuard;

use crate::types::Data;
use crate::types::PoiseContext;
use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude as serenity;
use serenity::ChannelId;
use serenity::EmojiId;
use serenity::Guild;
use serenity::Message;
use serenity::Reaction;
use serenity::ReactionType;

pub async fn reaction_management(
    ctx: &serenity::Context,
    data: &Data,
    reaction: &Reaction,
) -> anyhow::Result<()> {
    let message = reaction.message(ctx).await?;

    starboard(ctx, data, &message, reaction).await?;

    Ok(())
}

pub async fn starboard(
    ctx: &serenity::Context,
    data: &Data,
    message: &Message,
    reaction: &Reaction,
) -> anyhow::Result<()> {
    let reaction_type = &reaction.emoji;

    let name = match reaction_type {
        ReactionType::Unicode(String) => String.to_owned(),
        ReactionType::Custom { id, .. } => id.as_u64().to_string(),
        _ => "Error".to_string(),
    };

    let mut reaction_count = 0;

    for message_reaction in &message.reactions {
        if message_reaction.reaction_type == *reaction_type {
            reaction_count = message_reaction.count;
            break;
        }
    }

    // TODO: When converting to config file constants, the emojis crate offers shortcode conversion
    if reaction_count > 0 && name == "⭐" {
        let starboard_channel = ChannelId(900962773599658055);

        let previous_messages = starboard_channel.messages(ctx, |m| m).await?;

        let mut send = true;

        for previous_message in previous_messages {
            if previous_message.content.contains(&message.link()) {
                send = false;
                break;
            }
        }

        if send {
            starboard_channel
                .send_message(ctx, |m| {
                    m.content(format!(
                        "{} \n {} \n {}",
                        message.content,
                        message.author,
                        message.link(),
                    ))
                })
                .await?;
        }
    }

    Ok(())
}

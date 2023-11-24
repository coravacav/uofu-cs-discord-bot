use crate::data::Data;

use anyhow::Context;
use poise::serenity_prelude as serenity;
use serenity::{ChannelId, Message, Reaction, ReactionType};

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
        ReactionType::Custom { id, .. } => id.as_u64().to_string(),
        _ => anyhow::bail!("Unknown reaction type"),
    };

    let reaction_count = message
        .reactions
        .iter()
        .find(|reaction| reaction.reaction_type == *reaction_type)
        .map_or(0, |reaction| reaction.count);

    let config = data.config.read().await;

    let starboard = config
        .starboards
        .iter()
        .find(|starboard| starboard.emote_name == name)
        .context("No starboard found for emote")?;

    if reaction_count < starboard.reaction_count {
        anyhow::bail!("Reaction count is too low");
    }

    let message_link = message.link();

    let starboard_channel = ChannelId(starboard.channel_id);
    let recent_messages = starboard_channel.messages(ctx, |m| m).await?;

    let has_already_been_added = recent_messages.iter().any(|message| {
        message.embeds.iter().any(|embed| {
            embed
                .description
                .as_ref()
                .is_some_and(|description| description.contains(&message_link))
        })
    });

    if has_already_been_added {
        anyhow::bail!("Message has already been added to starboard");
    }

    starboard_channel
        .send_message(ctx, |m| {
            m.add_embed(|embed| {
                embed.description(format!("{}\n{}", message.content, message.link()));

                embed.author(|author| {
                    author.name(&message.author.name).icon_url(
                        message
                            .author
                            .avatar_url()
                            .as_deref()
                            .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png"),
                    )
                });

                embed.timestamp(message.timestamp);

                if let Some(attachment) = message.attachments.iter().find(|attachment| {
                    attachment
                        .content_type
                        .as_ref()
                        .is_some_and(|content_type| content_type.starts_with("image"))
                }) {
                    embed.image(&attachment.url);
                }

                embed
            })
        })
        .await?;

    Ok(())
}

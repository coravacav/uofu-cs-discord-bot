use crate::data::Data;

use poise::serenity_prelude as serenity;
use serenity::ChannelId;
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
        ReactionType::Unicode(string) => emojis::get(string)
            .expect("Default emojis should always be in unicode")
            .name()
            .to_owned(),
        ReactionType::Custom { id, .. } => id.as_u64().to_string(),
        _ => "Error".to_owned(),
    };

    let reaction_count = message
        .reactions
        .iter()
        .find(|reaction| reaction.reaction_type == *reaction_type)
        .map_or(0, |reaction| reaction.count);

    let config = data.config.read().await;

    let reaction_count_requirement = *config.get_starboard_reaction_count();
    let stored_name = config.get_starboard_emote();
    let starboard_channel = ChannelId(*config.get_starboard_channel());

    if reaction_count >= reaction_count_requirement && &name == stored_name {
        let previous_messages = starboard_channel.messages(ctx, |m| m).await?;

        if !previous_messages.iter().any(|message| {
            message.embeds.iter().any(|embed| {
                embed
                    .description
                    .as_ref()
                    .is_some_and(|description| description.contains(&message.link()))
            })
        }) {
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
        }
    }

    Ok(())
}

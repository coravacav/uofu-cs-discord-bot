use crate::types::Data;

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
        _ => "Error".to_string(),
    };

    let mut reaction_count = 0;

    for message_reaction in &message.reactions {
        if message_reaction.reaction_type == *reaction_type {
            reaction_count = message_reaction.count;
            break;
        }
    }

    let reaction_count_requirement = *data.config.get_starboard_reaction_count();
    let stored_name = (*data.config.get_starboard_emote()).clone();
    let starboard_channel = ChannelId(*data.config.get_starboard_channel());

    if reaction_count > reaction_count_requirement && name == stored_name {
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

use std::sync::Arc;

use crate::{
    commands::{handle_message_limit_interaction, handle_yeeting},
    data::State,
    handle_starboards::handle_starboards,
    text_detection::{kingfisher_reply_reactions, text_detection_and_reaction},
};
use bot_traits::ForwardRefToTracing;
use color_eyre::eyre::Result;
use futures::StreamExt;
use poise::serenity_prelude::{self as serenity, CreateMessage};

pub async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    data: State,
) -> Result<()> {
    match event {
        serenity::FullEvent::Message { new_message } => {
            // Wordle bot detection
            if new_message.author.id == 1211781489931452447
                && new_message.channel_id != 1397342642617978920
            {
                let channel_id = new_message.channel_id;
                let _ = new_message.delete(ctx).await;

                // See if the last message was kingfisher saying Please use the wordle bot and skip sending another if so
                let kingfisher_already_said = channel_id
                    .messages_iter(ctx)
                    .take(2)
                    .any(|message| async move {
                        message.is_ok_and(|message| {
                            message.author.id == ctx.cache.current_user().id
                                && message.content.starts_with("Please use the wordle bot in")
                        })
                    })
                    .await;

                if kingfisher_already_said {
                    return Ok(());
                }

                channel_id
                    .send_message(
                        ctx,
                        CreateMessage::new()
                            .content("Please use the wordle bot in <#1397342642617978920>"),
                    )
                    .await?;

                return Ok(());
            }

            // Track message for limit enforcement
            crate::track_message_for_limit(ctx, new_message)
                .await
                .trace_err_ok();

            text_detection_and_reaction(ctx, data, new_message)
                .await
                .trace_err_ok();
        }
        serenity::FullEvent::ReactionAdd {
            add_reaction: reaction,
        } => {
            let Ok(message) = reaction.message(ctx).await else {
                let message_link = format!(
                    "https://discord.com/channels/{}/{}/{}",
                    reaction.guild_id.map(|id| id.get()).unwrap_or(0),
                    reaction.channel_id,
                    reaction.message_id
                );

                tracing::warn!("Failed to get message! {:?}", message_link);
                return Ok(());
            };

            let message = Arc::new(message);

            {
                let ctx = ctx.clone();
                let data = data.clone();
                let message = message.clone();
                tokio::spawn(
                    async move { handle_yeeting(&ctx, data, &message).await.trace_err_ok() },
                );
            }

            {
                let ctx = ctx.clone();
                let reaction_user = reaction.user_id;
                let reaction = reaction.emoji.clone();

                tokio::spawn(async move {
                    kingfisher_reply_reactions(&ctx, reaction_user.as_ref(), reaction).await
                });
            }

            handle_starboards(ctx, data, &message, reaction)
                .await
                .trace_err_ok();
        }
        serenity::FullEvent::InteractionCreate { interaction } => {
            if let serenity::Interaction::Component(component) = interaction {
                handle_message_limit_interaction(ctx, component)
                    .await
                    .trace_err_ok();
            }
        }
        serenity::FullEvent::Ratelimit { data } => {
            tracing::warn!("Ratelimited: {:?}", data);
        }
        _ => {}
    };

    Ok(())
}

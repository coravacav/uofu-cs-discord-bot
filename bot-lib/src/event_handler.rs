use std::sync::Arc;

use crate::{
    commands::handle_yeeting,
    data::State,
    handle_starboards::handle_starboards,
    text_detection::{delete_message_if_user_trashcans, text_detection_and_reaction},
};
use bot_traits::ForwardRefToTracing;
use color_eyre::eyre::Result;
use poise::serenity_prelude as serenity;

pub async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    data: State,
) -> Result<()> {
    match event {
        serenity::FullEvent::Message { new_message } => {
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
                    delete_message_if_user_trashcans(&ctx, reaction_user.as_ref(), reaction).await
                });
            }

            handle_starboards(ctx, data, &message, reaction)
                .await
                .trace_err_ok();
        }
        serenity::FullEvent::Ratelimit { data } => {
            tracing::warn!("Ratelimited: {:?}", data);
        }
        _ => {}
    };

    Ok(())
}

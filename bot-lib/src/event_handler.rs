use crate::{
    commands::handle_yeeting, data::AppState, handle_starboards::handle_starboards,
    text_detection::text_detection,
};
use color_eyre::eyre::{Error, Result};
use poise::serenity_prelude as serenity;
use tap::Pipe;

pub async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    framework: poise::FrameworkContext<'_, AppState, Error>,
    _data: &AppState,
) -> Result<()> {
    if let Err(e) = match event {
        serenity::FullEvent::Message { new_message } => {
            let message_text = &new_message.content;
            let message_link = &new_message.link();

            tracing::trace!("message {} received {}", message_text, message_link);

            text_detection(ctx, framework.user_data, new_message).await
        }
        serenity::FullEvent::ReactionAdd {
            add_reaction: reaction,
        } => {
            // https://discord.com/channels/1065373537591894086/1065373538258800662/1288309958898880549
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

            tokio::join!(
                handle_yeeting(ctx, framework.user_data, &message),
                handle_starboards(ctx, framework.user_data, &message, reaction)
            )
            .pipe(|(err1, err2)| match (err1, err2) {
                (Err(e), _) => Err(e),
                (_, Err(e)) => Err(e),
                _ => Ok(()),
            })
        }
        serenity::FullEvent::Ratelimit { data } => {
            tracing::warn!("Ratelimited: {:?}", data);
            Ok(())
        }
        _ => Ok(()),
    } {
        tracing::error!("Error in event handler: {:?}", e);
    }

    Ok(())
}

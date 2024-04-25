use crate::{
    commands::lynch::handle_lynching, data::AppState, handle_starboards::handle_starboards,
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
            let message = reaction.message(ctx).await?;

            if tracing::event_enabled!(tracing::Level::TRACE) {
                let reaction_text = match reaction.emoji {
                    serenity::ReactionType::Unicode(ref string) => emojis::get(string)
                        .map(|emoji| emoji.name().to_owned())
                        .unwrap_or(string.to_owned()),
                    serenity::ReactionType::Custom { id, .. } => id.to_string(),
                    _ => format!("{:?}", reaction.emoji),
                };

                let message_text = &message.content;

                tracing::trace!(
                    "reaction {:?} added to message {:?}",
                    reaction_text,
                    message_text
                );
            }

            tokio::join!(
                handle_lynching(ctx, &message),
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

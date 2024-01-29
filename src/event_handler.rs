use crate::{data::AppState, handle_starboards::handle_starboards, text_detection::text_detection};
use color_eyre::eyre::{Error, Result};
use colored::Colorize;
use poise::serenity_prelude as serenity;

pub async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    framework: poise::FrameworkContext<'_, AppState, Error>,
    _data: &AppState,
) -> Result<()> {
    match event {
        serenity::FullEvent::Message { new_message } => {
            text_detection(ctx, framework.user_data, new_message).await
        }
        serenity::FullEvent::ReactionAdd {
            add_reaction: reaction,
        }
        | serenity::FullEvent::ReactionRemove {
            removed_reaction: reaction,
        } => {
            let message = reaction.message(ctx).await?;
            handle_starboards(ctx, framework.user_data, &message, reaction).await
        }
        serenity::FullEvent::Ratelimit { data } => {
            println!("{} {:?}", "Ratelimited:".yellow(), data);
            Ok(())
        }
        _ => Ok(()),
    }
}

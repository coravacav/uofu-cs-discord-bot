use crate::{data::State, starboard::Starboard};
use color_eyre::eyre::Result;
use poise::serenity_prelude::{Context, Message, Reaction, Timestamp};

pub async fn handle_starboards(
    ctx: &Context,
    data: State,
    message: &Message,
    reaction: &Reaction,
) -> Result<()> {
    if is_message_too_recent(&message.timestamp) || is_message_yeet(message) {
        return Ok(());
    }

    let config = data.config.read().await;

    let futures = config.starboards.iter().map(|starboard| async {
        if Starboard::has_recent_message(message.id)
            .await
            .unwrap_or(false)
        {
            return;
        }

        if !starboard.does_starboard_apply(message, reaction) {
            return;
        }

        Starboard::insert_recent_message(message.id)
            .await
            .unwrap_or_else(|_| tracing::error!("Failed to insert recent message {}", message.id));

        starboard.reply(ctx, message, &reaction.emoji).await.ok();
    });

    futures::future::join_all(futures).await;

    Ok(())
}

fn is_message_too_recent(message_timestamp: &Timestamp) -> bool {
    message_timestamp.unix_timestamp()
        < (chrono::Utc::now() - chrono::TimeDelta::weeks(1)).timestamp()
}

fn is_message_yeet(message: &Message) -> bool {
    crate::commands::YEET_STARBOARD_EXCLUSIONS
        .lock()
        .contains(&message.id)
}

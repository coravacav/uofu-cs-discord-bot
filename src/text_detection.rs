use std::sync::Arc;

use crate::types::Data;
use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude as serenity;
use serenity::Message;

pub async fn text_detection(
    ctx: &serenity::Context,
    data: &Data,
    message: &Message,
) -> anyhow::Result<()> {
    if message.is_own(ctx) {
        return Ok(());
    }

    if let Some(name) = &data.check_should_respond(message).await {
        if cooldown_checker(
            data.last_response(Arc::clone(name)),
            data.config.read().await.get_cooldown(),
            message.timestamp.with_timezone(&Utc),
        ) {
            data.reset_last_response(Arc::clone(name), message.timestamp.with_timezone(&Utc));
            data.run_action(name, message, ctx).await?;
        }
    }

    Ok(())
}

/// Checks if the cooldown is met. If yes, it is, returns true and resets the cooldown. If not,
/// returns false and does nothing.
fn cooldown_checker(
    last_message: Option<DateTime<Utc>>,
    cooldown: &Duration,
    timestamp: DateTime<Utc>,
) -> bool {
    if let Some(last_message) = last_message {
        last_message + *cooldown <= timestamp
    } else {
        false
    }
}

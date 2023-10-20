use crate::types::{Data, Error};

use std::sync::{Mutex, MutexGuard};

use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude as serenity;
use poise::Event;
use serenity::Message;

pub async fn text_detection(
    ctx: &serenity::Context,
    _event: &Event<'_>,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
    message: &Message,
) -> Result<(), Error> {
    if message.is_own(ctx) {
        return Ok(());
    }
    if let Some(name) = data.check_should_respond(message) {
        if cooldown_checker(
            data.last_response(&name),
            data.config.lock_cooldown(),
            message.timestamp.with_timezone(&Utc),
        ) {
            data.reset_last_response(&name, message.timestamp.with_timezone(&Utc));
            data.run_action(&name, message, ctx).await?;
        }
    }

    Ok(())
}

/// Checks if the cooldown is met. If yes, it is, returns true and resets the cooldown. If not,
/// returns false and does nothing.
fn cooldown_checker(
    last_message: DateTime<Utc>,
    cooldown: MutexGuard<Duration>,
    timestamp: DateTime<Utc>,
) -> bool {
    if last_message + *cooldown > timestamp {
        return false;
    }

    true
}

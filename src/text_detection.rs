use crate::types;
use poise::serenity_prelude as serenity;
use poise::Event;
use serenity::Message;

use types::Data;
use types::Error;

pub async fn text_detection(
    ctx: &serenity::Context,
    event: &Event<'_>,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
    message: &Message,
) -> Result<(), Error> {
    if message.content.to_lowercase().contains("rust") && !message.author.bot {
        message
            .reply(ctx, format!("RUST MENTIONED :crab: :crab: :crab:"))
            .await?;
    }

    return Ok(());
}
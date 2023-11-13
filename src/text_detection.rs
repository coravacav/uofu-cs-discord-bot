use crate::data::Data;
use anyhow::Context;
use poise::serenity_prelude::{self as serenity};
use serenity::Message;

pub async fn text_detection(
    ctx: &serenity::Context,
    data: &Data,
    message: &Message,
) -> anyhow::Result<()> {
    if message.is_own(ctx) {
        return Ok(());
    }

    if !message
        .author
        .has_role(
            ctx,
            message.guild_id.context("should have guild id")?,
            *data.config.read().await.get_bot_react_role_id(),
        )
        .await
        .context("Couldn't get roles")?
    {
        println!("Message {} doesn't have bot react role", message.author);
        return Ok(());
    }

    if let Some(message_response) = data.find_response(&message.content).await {
        data.run_action(&message_response, message, ctx).await?;
    }

    Ok(())
}

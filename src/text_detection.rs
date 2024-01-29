use crate::data::AppState;
use color_eyre::eyre::{Context, OptionExt, Result};
use poise::serenity_prelude::{self as serenity};
use serenity::Message;

pub async fn text_detection(
    ctx: &serenity::Context,
    data: &AppState,
    message: &Message,
) -> Result<()> {
    if message.is_own(ctx) {
        return Ok(());
    }

    if !message
        .author
        .has_role(
            ctx,
            message.guild_id.ok_or_eyre("should have guild id")?,
            data.config.read().await.bot_react_role_id,
        )
        .await
        .context("Couldn't get roles")?
    {
        return Ok(());
    }

    if let Some(message_response) = data.find_response(&message.content, &message.link()).await {
        data.run_action(&message_response, message, ctx).await?;
    }

    Ok(())
}

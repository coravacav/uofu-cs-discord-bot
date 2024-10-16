use crate::data::AppState;
use color_eyre::eyre::{OptionExt, Result};
use poise::serenity_prelude::{Context, Message};

#[tracing::instrument(level = "trace", skip(ctx, data))]
pub async fn text_detection(ctx: &Context, data: &AppState, message: &Message) -> Result<()> {
    if message.author == **ctx.cache.current_user() {
        return Ok(());
    }

    if message.author.bot {
        return Ok(());
    }

    let author = &message.author;
    let guild_id = message.guild_id.ok_or_eyre("should have guild id")?;

    let author_has_role = author
        .has_role(ctx, guild_id, data.config.read().await.bot_react_role_id)
        .await?;

    if !author_has_role {
        let author_name = &message.author.name;

        tracing::event!(
            tracing::Level::DEBUG,
            "User {} doesn't have the bot react role",
            author_name
        );

        return Ok(());
    }

    if let Some(message_response) = data.find_response(&message.content, &message.link()).await {
        data.respond(&message_response, message, ctx).await?;
    }

    Ok(())
}

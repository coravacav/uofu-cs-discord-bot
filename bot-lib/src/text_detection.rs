use crate::{config::ResponseKind, data::State};
use color_eyre::eyre::{OptionExt, Result};
use poise::serenity_prelude::{Context, Message};
use rand::seq::SliceRandom;

#[tracing::instrument(level = "trace", skip(ctx, data))]
pub async fn text_detection(ctx: &Context, data: State, message: &Message) -> Result<()> {
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

    let config = data.config.read().await;

    if let Some(response) = config.responses.iter().find_map(|response| {
        response.find_valid_response(&message.content, &config, &message.link())
    }) {
        match &*response {
            ResponseKind::Text { content } => {
                message.reply(ctx, content).await?;
            }
            ResponseKind::RandomText { content } => {
                let response = content
                    .choose(&mut rand::thread_rng())
                    .ok_or_eyre("The responses list is empty")?;

                message.reply(ctx, response).await?;
            }
            ResponseKind::None => {}
        }
    };

    Ok(())
}

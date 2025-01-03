use crate::{config::ResponseKind, data::State};
use color_eyre::eyre::{OptionExt, Result, bail};
use poise::serenity_prelude::{Context, Message};
use rand::seq::SliceRandom;

pub async fn text_detection(ctx: &Context, data: State, message: &Message) -> Result<()> {
    if message.author == **ctx.cache.current_user() || message.author.bot {
        return Ok(());
    }

    let author = &message.author;
    let guild_id = message.guild_id.ok_or_eyre("should have guild id")?;

    let author_has_role = author
        .has_role(
            ctx,
            guild_id,
            data.config.read().await.ids.bot_react_role_id,
        )
        .await?;

    if !author_has_role {
        return Ok(());
    }

    let config = data.config.read().await;

    if let Some(response) = config
        .ruleset_combinator
        .find_iter(&message.content)
        .filter_map(|name| match config.responses.get(&name) {
            Some(response) => Some(response),
            None => {
                tracing::error!("Response {} not found, this shouldn't happen", name);
                None
            }
        })
        .filter_map(|response| response.can_send(&message.content, &config))
        .next()
    {
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
            ResponseKind::None => {
                bail!("No response found for message");
            }
        }
    };

    Ok(())
}

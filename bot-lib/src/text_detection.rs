use std::sync::LazyLock;

use crate::{config::ResponseKind, data::State};
use ahash::AHashMap;
use bot_traits::ForwardRefToTracing;
use color_eyre::eyre::{OptionExt, Result, bail};
use parking_lot::Mutex;
use poise::serenity_prelude::{ChannelId, Context, Message, MessageId, ReactionType, UserId};
use rand::seq::SliceRandom;

static REPLY_TO_DELETE_LOOKUP: LazyLock<Mutex<AHashMap<UserId, (MessageId, ChannelId)>>> =
    LazyLock::new(|| Mutex::new(AHashMap::new()));

pub async fn text_detection_and_reaction(
    ctx: &Context,
    data: State,
    message: &Message,
) -> Result<()> {
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
        let response_message = match &*response {
            ResponseKind::Text { content } => message.reply(ctx, content).await?,
            ResponseKind::RandomText { content } => {
                let response = content
                    .choose(&mut rand::thread_rng())
                    .ok_or_eyre("The responses list is empty")?;

                message.reply(ctx, response).await?
            }
            ResponseKind::None => {
                bail!("No response found for message");
            }
        };

        {
            REPLY_TO_DELETE_LOOKUP.lock().insert(
                message.author.id,
                (response_message.id, response_message.channel_id),
            );
        }

        let reaction = ReactionType::Unicode("🗑️".to_string());

        response_message.react(&ctx, reaction.clone()).await?;

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        response_message
            .delete_reaction_emoji(&ctx, reaction)
            .await
            .ok();
    };

    Ok(())
}

pub async fn delete_message_if_user_trashcans(
    ctx: &Context,
    reaction_user: Option<&UserId>,
    reaction: ReactionType,
) {
    if !reaction.unicode_eq("🗑️") {
        return;
    }

    let Some(reaction_user) = reaction_user else {
        return;
    };

    let Some(&(message_id, channel_id)) = REPLY_TO_DELETE_LOOKUP.lock().get(reaction_user) else {
        return;
    };

    channel_id
        .delete_message(&ctx, message_id)
        .await
        .trace_err_ok();
}

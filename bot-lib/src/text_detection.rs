use std::sync::{Arc, LazyLock};

use crate::{config::ResponseKind, data::State};
use ahash::AHashMap;
use bot_traits::ForwardRefToTracing;
use color_eyre::eyre::{OptionExt, Result, bail};
use parking_lot::Mutex;
use poise::serenity_prelude::{ChannelId, Context, Message, MessageId, ReactionType, UserId};

#[derive(Clone, Debug)]
pub struct KingfisherReplyMetadata {
    pub message_id: MessageId,
    pub channel_id: ChannelId,
    pub response: Arc<ResponseKind>,
}

pub static KINGFISHER_REPLY_LAST_BY_USER: LazyLock<
    Mutex<AHashMap<UserId, KingfisherReplyMetadata>>,
> = LazyLock::new(|| Mutex::new(AHashMap::new()));

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
        let Some(response_text) = response.get_reply_text() else {
            bail!("No response found for message");
        };

        let response_message = message.reply(&ctx, response_text.as_ref()).await?;

        {
            KINGFISHER_REPLY_LAST_BY_USER.lock().insert(
                message.author.id,
                KingfisherReplyMetadata {
                    message_id: response_message.id,
                    channel_id: response_message.channel_id,
                    response: Arc::clone(&response.clone()),
                },
            );
        }

        response_message
            .react(&ctx, ReactionType::Unicode("🗑️".to_string()))
            .await?;

        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        response_message
            .delete_reaction_emoji(&ctx, ReactionType::Unicode("🗑️".to_string()))
            .await
            .ok();
    };

    Ok(())
}

pub async fn kingfisher_reply_reactions(
    ctx: &Context,
    reaction_user: Option<&UserId>,
    reaction: ReactionType,
) {
    let Some(reaction_user) = reaction_user else {
        return;
    };

    match reaction {
        ReactionType::Unicode(u) if u == "🗑️" => {
            let Some(&KingfisherReplyMetadata {
                message_id,
                channel_id,
                ..
            }) = KINGFISHER_REPLY_LAST_BY_USER.lock().get(reaction_user)
            else {
                return;
            };

            channel_id
                .delete_message(&ctx, message_id)
                .await
                .trace_err_ok();
        }
        _ => {}
    }
}

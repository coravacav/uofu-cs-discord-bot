use crate::data::PoiseContext;
use color_eyre::eyre::{bail, Context, OptionExt, Result};
use dashmap::{try_result::TryResult, DashMap};
use poise::{
    serenity_prelude::{self as serenity, ChannelId, UserId},
    CreateReply,
};
use std::{
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

/// This static is used to track when you're in the bots-channel
static INSTANT_BY_USER_ID: LazyLock<DashMap<UserId, Instant>> = LazyLock::new(DashMap::new);
const BOTS_CHANNEL_PER_USER_TIMEOUT_SECONDS: u64 = 60;
/// This static is used to track all non-bots-channel messages
static INSTANT_BY_CHANNEL_ID: LazyLock<DashMap<ChannelId, Instant>> = LazyLock::new(DashMap::new);
const PER_CHANNEL_TIMEOUT_SECONDS: u64 = 600;

const BOTS_CHANNEL_CHANNEL_ID: ChannelId = ChannelId::new(1105683912715415652);

async fn try_lock_llm(ctx: PoiseContext<'_>) -> Result<()> {
    let channel_id = ctx.channel_id();
    let user_id = ctx.author().id;

    if channel_id == BOTS_CHANNEL_CHANNEL_ID {
        match INSTANT_BY_USER_ID.try_get(&user_id) {
            TryResult::Locked => {
                let reply = CreateReply::default()
                    .ephemeral(true)
                    .content("Are you spamming?");
                ctx.send(reply).await?;
                bail!("Are you spamming?");
            }
            TryResult::Present(last_time)
                if last_time.elapsed()
                    < Duration::from_secs(BOTS_CHANNEL_PER_USER_TIMEOUT_SECONDS) =>
            {
                let reply = CreateReply::default()
                    .ephemeral(true)
                    .content("Please wait a minute before asking again");
                ctx.send(reply).await?;
                bail!("Please wait a minute before asking again");
            }
            _ => {}
        }

        INSTANT_BY_USER_ID.insert(user_id, Instant::now());
    } else {
        match INSTANT_BY_CHANNEL_ID.try_get(&channel_id) {
            TryResult::Locked => {
                let reply = CreateReply::default()
                    .ephemeral(true)
                    .content("Are you spamming?");
                ctx.send(reply).await?;
                bail!("Are you spamming?");
            }
            TryResult::Present(last_time)
                if last_time.elapsed() < Duration::from_secs(PER_CHANNEL_TIMEOUT_SECONDS) =>
            {
                let reply = CreateReply::default()
                    .ephemeral(true)
                    .content("Please wait 10 minutes before asking again");
                ctx.send(reply).await?;
                bail!("Please wait 10 minutes before asking again");
            }
            _ => {}
        }

        INSTANT_BY_CHANNEL_ID.insert(channel_id, Instant::now());
    }

    Ok(())
}

/// Ask kingfisher anything!
#[poise::command(slash_command, prefix_command, rename = "llm")]
pub async fn llm_prompt(ctx: PoiseContext<'_>, prompt: String) -> Result<()> {
    if try_lock_llm(ctx).await.ok().is_none() {
        return Ok(());
    }

    let prompt = Arc::new(prompt);

    ctx.defer().await?;

    let (reply, reply_rx) = tokio::sync::oneshot::channel();
    ctx.data()
        .llm_tx
        .send((Arc::clone(&prompt), reply))
        .wrap_err("Failed to send LLM task")?;

    let reply = reply_rx.await.wrap_err("LLM task failed")?;
    let guild_id = ctx.guild_id().ok_or_eyre("Couldn't get guild id")?;

    let author_username = ctx.author().name.clone();
    let author_nickname = ctx.author().nick_in(&ctx, guild_id).await;
    let shown_username: String = match author_nickname {
        Some(nickname) => format!("{} ({})", nickname, author_username),
        None => author_username,
    };

    let mut title = format!("{} asked, \"{}\"", shown_username, prompt);
    title.truncate(256); // Discord limits titles to 256 characters

    ctx.send(
        CreateReply::default()
            .embed(serenity::CreateEmbed::new().title(title).description(reply))
            .reply(true),
    )
    .await?;

    Ok(())
}

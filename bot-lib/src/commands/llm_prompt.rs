use crate::data::PoiseContext;
use color_eyre::eyre::{Context, OptionExt, Result};
use dashmap::{try_result::TryResult, DashMap};
use poise::{
    serenity_prelude::{self as serenity, UserId},
    CreateReply,
};
use std::{
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};

static LAST_REQUESTED_BY_USERID: LazyLock<DashMap<UserId, Instant>> = LazyLock::new(DashMap::new);

#[poise::command(slash_command, prefix_command, rename = "llm")]
pub async fn llm_prompt(ctx: PoiseContext<'_>, prompt: String) -> Result<()> {
    let prompt = Arc::new(prompt);
    let user_id = ctx.author().id;

    match LAST_REQUESTED_BY_USERID.try_get(&user_id) {
        TryResult::Locked => {
            let reply = CreateReply::default()
                .ephemeral(true)
                .content("Are you spamming?");
            ctx.send(reply).await?;
            return Ok(());
        }
        TryResult::Present(last_time) => {
            if last_time.elapsed() < Duration::from_secs(60) {
                let reply = CreateReply::default()
                    .ephemeral(true)
                    .content("Please wait a minute before asking again");
                ctx.send(reply).await?;
                return Ok(());
            }
        }
        _ => {}
    }

    LAST_REQUESTED_BY_USERID.insert(user_id, Instant::now());
    ctx.defer().await?;

    let (reply, reply_rx) = tokio::sync::oneshot::channel();
    ctx.data()
        .llm_tx
        .send((Arc::clone(&prompt), reply))
        .wrap_err("Failed to send LLM task")?;

    let reply = reply_rx.await.wrap_err("LLM task failed")?;
    let guild_id = ctx.guild_id().ok_or_eyre("Couldn't get guild id")?;

    ctx.send(
        CreateReply::default()
            .embed(
                serenity::CreateEmbed::new()
                    .title(format!(
                        "{} asked, \"{}\"",
                        ctx.author()
                            .nick_in(&ctx, guild_id)
                            .await
                            .ok_or_eyre("Couldn't get nick")?,
                        prompt
                    ))
                    .description(reply),
            )
            .reply(true),
    )
    .await?;

    Ok(())
}

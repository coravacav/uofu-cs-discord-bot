use crate::{data::PoiseContext, utils::GetRelativeTimestamp};
use color_eyre::eyre::{ContextCompat, Result};
use humantime::parse_duration;
use poise::serenity_prelude::{EditMember, Mentionable};
use std::time::Duration;

#[poise::command(slash_command, ephemeral = true)]
pub async fn timeout(
    ctx: PoiseContext<'_>,
    #[description = "The amount of time to time yourself out, like '1h' or '3m'"] time_text: String,
    #[description = "Hide the announcement of how long you'll be timed out"]
    hide_notification: Option<bool>,
) -> Result<()> {
    if time_text.len() > 20 {
        ctx.say("Send something reasonable, please.").await?;
        return Ok(());
    }

    let author = ctx.author();
    let Ok(time) = parse_duration(&time_text) else {
        ctx.say("Invalid time format! Say something like '1h' or '3m'")
            .await?;

        return Ok(());
    };

    if time > Duration::from_secs(60 * 60 * 24 * 28) {
        ctx.say("Discord isn't cool and doesn't let you time out for more than 28 days")
            .await?;
        return Ok(());
    }

    if time < Duration::from_secs(1) {
        ctx.say("_huh_").await?;
        return Ok(());
    }

    let timeout_end = chrono::Utc::now() + time;

    let guild_id = ctx.guild_id().wrap_err("No guild ID?")?;

    if guild_id
        .edit_member(
            ctx,
            author.id,
            EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
        )
        .await
        .is_err()
    {
        ctx.say("Failed to time out! Ur too powerful :(").await?;
        return Ok(());
    };

    tracing::info!(
        "{} timed out until {} ({})",
        author.tag(),
        timeout_end,
        time_text
    );

    if let Some(true) = hide_notification {
        return Ok(());
    }

    // Skip the notification if the timeout is less than 3 seconds to avoid flickering
    if time < std::time::Duration::from_secs(3) {
        return Ok(());
    }

    let reply_handle = ctx
        .say(format!(
            "{} has timed themselves out. They will return {}",
            author.mention(),
            // Snippet to get nick < global name < name
            // guild_id
            //     .member(ctx, author.id)
            //     .await?
            //     .nick
            //     .unwrap_or_else(|| {
            //         author
            //             .global_name
            //             .clone()
            //             .unwrap_or_else(|| author.name.clone())
            //     }),
            timeout_end.discord_relative_timestamp(),
        ))
        .await?;

    tokio::time::sleep(time - std::time::Duration::from_secs(1)).await;

    reply_handle.delete(ctx).await.ok();

    Ok(())
}

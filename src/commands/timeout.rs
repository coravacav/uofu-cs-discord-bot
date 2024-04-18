use crate::data::PoiseContext;
use color_eyre::eyre::{ContextCompat, Result, WrapErr};
use poise::{
    serenity_prelude::{self as serenity, Mentionable},
    CreateReply,
};

#[poise::command(slash_command, prefix_command)]
pub async fn timeout(
    ctx: PoiseContext<'_>,
    #[description = "The amount of time to time yourself out, like '1h' or '3m'"] time_text: String,
    #[description = "Hide the announcement of how long you'll be timed out"]
    hide_notification: Option<bool>,
) -> Result<()> {
    tracing::trace!("timeout command");

    let author = ctx.author();
    let Ok(time) = fundu::parse_duration(&time_text) else {
        tracing::info!(
            "{} tried to time out with invalid time '{}'",
            author.tag(),
            time_text
        );

        ctx.say("Invalid time format! Say something like '1h' or '3m'")
            .await?;

        return Ok(());
    };

    let timeout_end = chrono::Utc::now() + time;

    let guild_id = ctx.guild_id().wrap_err("No guild ID?")?;

    let Ok(_) = guild_id
        .edit_member(
            ctx,
            author.id,
            serenity::EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
        )
        .await
        .wrap_err("Failed to edit member")
    else {
        ctx.say("Failed to time out! Ur too powerful :(").await?;
        return Ok(());
    };

    tracing::info!(
        "{} timed out until {} ({})",
        author.tag(),
        timeout_end,
        time_text
    );

    if ctx.prefix() == "/" {
        ctx.send(
            CreateReply::default()
                .ephemeral(true)
                .content(format!("Timed out until {}", timeout_end)),
        )
        .await?;
    }

    if let Some(true) = hide_notification {
        return Ok(());
    }

    if time < std::time::Duration::from_secs(3) {
        return Ok(());
    }

    let reply_handle = ctx
        .say(format!(
            "{} has timed themselves out. They will return <t:{}:R>",
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
            timeout_end.timestamp()
        ))
        .await?;

    tracing::trace!("Announced timeout");

    tokio::time::sleep(time - std::time::Duration::from_secs(1)).await;

    reply_handle
        .delete(ctx)
        .await
        .wrap_err("Failed to delete message")?;

    Ok(())
}

use crate::data::PoiseContext;
use color_eyre::eyre::{Context, ContextCompat, Result};
use poise::{serenity_prelude as serenity, CreateReply};

#[poise::command(slash_command, prefix_command)]
pub async fn timeout(
    ctx: PoiseContext<'_>,
    #[description = "The amount of time to time yourself out, like '1h' or '3m'"] time_text: String,
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

    let guild_id = ctx.guild_id().context("No guild ID?")?;

    guild_id
        .edit_member(
            ctx,
            author.id,
            serenity::EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
        )
        .await
        .context("Failed to edit member")?;

    tracing::info!(
        "{} timed out until {} ({})",
        author.tag(),
        timeout_end,
        time_text
    );

    ctx.send(
        CreateReply::default()
            .ephemeral(true)
            .content(format!("Timed out until {}", timeout_end)),
    )
    .await?;

    Ok(())
}

use crate::data::PoiseContext;
use color_eyre::eyre::Result;

/// Command to easily create a github issue for Kingfisher
#[poise::command(slash_command, ephemeral = true)]
pub async fn send_feedback(ctx: PoiseContext<'_>) -> Result<()> {
    ctx.say("<https://github.com/coravacav/uofu-cs-discord-bot/issues/new>")
        .await?;
    Ok(())
}

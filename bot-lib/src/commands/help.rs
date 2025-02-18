use crate::data::PoiseContext;
use color_eyre::eyre::Result;

/// Displays bot info and available commands
#[poise::command(slash_command, ephemeral = true)]
pub async fn help(ctx: PoiseContext<'_>) -> Result<()> {
    let help_text = ctx.data().config.read().await.help_text.clone();

    match help_text {
        Some(help_text) => {
            ctx.say(&*help_text).await?;
        }
        None => {
            ctx.say("Help text could not be found. Please contact the bot owner to set it up.")
                .await?;
        }
    }

    Ok(())
}

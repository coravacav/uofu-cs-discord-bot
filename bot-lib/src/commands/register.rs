use crate::data::PoiseContext;
use color_eyre::eyre::Result;
use poise::builtins::register_application_commands_buttons;

#[poise::command(prefix_command)]
pub async fn register(ctx: PoiseContext<'_>) -> Result<()> {
    register_application_commands_buttons(ctx).await?;
    Ok(())
}

use crate::data::PoiseContext;
use poise::builtins::register_application_commands_buttons;

#[poise::command(prefix_command)]
pub async fn register(ctx: PoiseContext<'_>) -> anyhow::Result<()> {
    register_application_commands_buttons(ctx).await?;
    Ok(())
}

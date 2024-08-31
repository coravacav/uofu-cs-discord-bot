use crate::{commands::is_stefan, data::PoiseContext};
use color_eyre::eyre::Result;
use poise::serenity_prelude::UserId;

#[poise::command(
    slash_command,
    subcommands(
        "clear_value",
        "inspect_value",
    ),
    check = is_stefan
)]
pub async fn db_admin(_ctx: PoiseContext<'_>) -> Result<()> {
    Ok(())
}

#[poise::command(slash_command, ephemeral = true, check=is_stefan)]
async fn clear_value(ctx: PoiseContext<'_>, tree: String, key: UserId) -> Result<()> {
    let key: u64 = key.into();
    ctx.data().db.debug_remove_value(&tree, &key)?;

    ctx.reply("Value cleared!").await?;

    Ok(())
}

#[poise::command(slash_command, ephemeral = true, check=is_stefan)]
async fn inspect_value(ctx: PoiseContext<'_>, tree: String, key: UserId) -> Result<()> {
    let key: u64 = key.into();
    let value = ctx.data().db.debug_inspect_value(&tree, &key)?;

    ctx.reply(format!("Value is {:?}", value)).await?;

    Ok(())
}

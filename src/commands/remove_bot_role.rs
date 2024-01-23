use crate::data::PoiseContext;

use anyhow::Context;
use poise::serenity_prelude::RoleId;

#[poise::command(slash_command, prefix_command, rename = "ignoreme")]
pub async fn remove_bot_role(ctx: PoiseContext<'_>) -> anyhow::Result<()> {
    let author = ctx.author();
    let guild = ctx.guild().context("Couldn't get guild")?.clone();

    guild
        .member(ctx, author.id)
        .await
        .context("Couldn't get member")?
        .remove_role(
            ctx,
            RoleId::from(ctx.data().config.read().await.bot_react_role_id),
        )
        .await
        .context("Couldn't remove role")?;

    ctx.say("Removed role!").await?;

    Ok(())
}

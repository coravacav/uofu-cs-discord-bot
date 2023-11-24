use crate::data::PoiseContext;

use anyhow::Context;
use poise::serenity_prelude::RoleId;

#[poise::command(slash_command, prefix_command, rename = "reactme")]
pub async fn add_bot_role(ctx: PoiseContext<'_>) -> anyhow::Result<()> {
    let author = ctx.author();
    let guild = ctx.guild().context("Couldn't get guild")?;

    guild
        .member(ctx, author.id)
        .await
        .context("Couldn't get member")?
        .add_role(
            ctx,
            RoleId::from(ctx.data().config.read().await.bot_react_role_id),
        )
        .await
        .context("Couldn't add role")?;

    ctx.say("Added role!").await?;

    Ok(())
}

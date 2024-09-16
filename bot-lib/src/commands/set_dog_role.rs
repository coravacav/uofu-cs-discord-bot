use crate::data::PoiseContext;
use color_eyre::eyre::{OptionExt, Result, WrapErr};
use poise::serenity_prelude::RoleId;

#[poise::command(slash_command, rename = "woof", ephemeral = true)]
pub async fn add_dog_role(ctx: PoiseContext<'_>) -> Result<()> {
    let author = ctx.author();
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let role_id = RoleId::from(ctx.data().config.read().await.dog_react_role_id);

    guild
        .member(ctx, author.id)
        .await
        .wrap_err("Couldn't get member")?
        .add_role(ctx, role_id)
        .await
        .wrap_err("Couldn't add role")?;

    ctx.say("Added role!").await?;

    Ok(())
}

#[poise::command(slash_command, rename = "muzzle", ephemeral = true)]
pub async fn remove_dog_role(ctx: PoiseContext<'_>) -> Result<()> {
    let author = ctx.author();
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let role_id = RoleId::from(ctx.data().config.read().await.dog_react_role_id);

    guild
        .member(ctx, author.id)
        .await
        .wrap_err("Couldn't get member")?
        .remove_role(ctx, role_id)
        .await
        .wrap_err("Couldn't remove role")?;

    ctx.say("Removed role!").await?;

    Ok(())
}

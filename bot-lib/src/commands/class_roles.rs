use crate::commands::{get_author, get_role};
use crate::data::PoiseContext;
use color_eyre::eyre::{Result, WrapErr};

#[poise::command(slash_command, prefix_command, rename = "join_class", ephemeral = true)]
pub async fn add_class_role(
    ctx: PoiseContext<'_>,
    #[description = "The class number, eg. for CS2420 put in \"2420\""] number: u32,
) -> Result<()> {
    let author = get_author(ctx).await?;
    let role_id = get_role(ctx, number).await?;

    author
        .add_role(ctx, role_id)
        .await
        .wrap_err("Couldn't remove role")?;

    ctx.say("Left class!").await?;

    Ok(())
}

#[poise::command(
    slash_command,
    prefix_command,
    rename = "leave_class",
    ephemeral = true
)]
pub async fn remove_class_role(
    ctx: PoiseContext<'_>,
    #[description = "The class number, eg. for CS2420 put in \"2420\""] number: u32,
) -> Result<()> {
    let author = get_author(ctx).await?;
    let role_id = get_role(ctx, number).await?;

    author
        .remove_role(ctx, role_id)
        .await
        .wrap_err("Couldn't remove role")?;

    ctx.say("Left class!").await?;

    Ok(())
}

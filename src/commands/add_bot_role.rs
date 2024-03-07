use crate::data::PoiseContext;
use color_eyre::eyre::{Context, OptionExt, Result};
use poise::serenity_prelude::RoleId;

#[poise::command(slash_command, prefix_command, rename = "reactme")]
pub async fn add_bot_role(ctx: PoiseContext<'_>) -> Result<()> {
    let author = ctx.author();
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.clone();
    let role_id = RoleId::from(ctx.data().config.read().await.bot_react_role_id);

    guild
        .member(ctx, author.id)
        .await
        .context("Couldn't get member")?
        .add_role(ctx, role_id)
        .await
        .context("Couldn't add role")?;
    {
        let members = &mut ctx
            .framework()
            .user_data
            .config
            .write()
            .await
            .bot_react_role_members;

        let author_id = author.id.into();

        members.retain(
        |member| matches!(member, crate::config::ReactRole { user_id, .. } if user_id != &author_id),
    );

        members.push(crate::config::ReactRole {
            user_id: author_id,
            react: true,
        });
    }

    ctx.say("Added role!").await?;

    Ok(())
}

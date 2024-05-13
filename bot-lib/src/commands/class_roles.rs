use crate::data::PoiseContext;
use color_eyre::eyre::{OptionExt, Result, WrapErr};

#[poise::command(slash_command, prefix_command, rename = "join_class")]
pub async fn add_class_role(
    ctx: PoiseContext<'_>,
    #[description = "The class number, eg. for CS2420 put in \"2420\""] number: u32,
) -> Result<()> {
    let author = ctx.author();
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let roles = guild.roles(ctx).await?;

    let role_name = format!("CS {}", number);
    let Some((role_id, _role)) = roles.iter().find(|x| x.1.name.contains(&role_name)) else {
        ctx.say("Couldn't find the class!").await?;
        return Ok(());
    };

    guild
        .member(ctx, author.id)
        .await
        .wrap_err("Couldn't get member")?
        .add_role(ctx, role_id)
        .await
        .wrap_err("Couldn't add role")?;
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

    ctx.say("Joined class!").await?;

    Ok(())
}

#[poise::command(slash_command, prefix_command, rename = "leave_class")]
pub async fn remove_class_role(
    ctx: PoiseContext<'_>,
    #[description = "The class number, eg. for CS2420 put in \"2420\""] number: u32,
) -> Result<()> {
    let author = ctx.author();
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let roles = guild.roles(ctx).await?;

    let role_name = format!("CS {}", number);
    let Some((role_id, _role)) = roles.iter().find(|x| x.1.name.contains(&role_name)) else {
        ctx.say("Couldn't find the class!").await?;
        return Ok(());
    };

    guild
        .member(ctx, author.id)
        .await
        .wrap_err("Couldn't get member")?
        .remove_role(ctx, role_id)
        .await
        .wrap_err("Couldn't remove role")?;

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
            react: false,
        });
    }

    ctx.say("Left class!").await?;

    Ok(())
}

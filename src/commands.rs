use crate::data::PoiseContext;

use anyhow::Context;
use chrono::Duration;
use poise::serenity_prelude::{self as serenity, RoleId};
use serenity::{ChannelType, PermissionOverwrite, PermissionOverwriteType, Permissions};

#[poise::command(slash_command)]
pub async fn change_text_detect_cooldown(
    ctx: PoiseContext<'_>,
    #[description = "The cooldown in minutes"] cooldown: i64,
) -> anyhow::Result<()> {
    ctx.data()
        .config
        .write()
        .await
        .update_cooldown(Duration::minutes(cooldown));
    ctx.say("Done!").await?;
    Ok(())
}

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
            RoleId::from(*ctx.data().config.read().await.get_bot_react_role_id()),
        )
        .await
        .context("Couldn't add role")?;

    ctx.say("Added role!").await?;

    Ok(())
}

#[poise::command(slash_command, prefix_command, rename = "ignoreme")]
pub async fn remove_bot_role(ctx: PoiseContext<'_>) -> anyhow::Result<()> {
    let author = ctx.author();
    let guild = ctx.guild().context("Couldn't get guild")?;

    guild
        .member(ctx, author.id)
        .await
        .context("Couldn't get member")?
        .remove_role(
            ctx,
            RoleId::from(*ctx.data().config.read().await.get_bot_react_role_id()),
        )
        .await
        .context("Couldn't remove role")?;

    ctx.say("Removed role!").await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn create_class_category(
    ctx: PoiseContext<'_>,
    #[description = "The class number, eg. for CS2420 put in \"2420\""] number: u32,
) -> anyhow::Result<()> {
    let guild = ctx.guild().context("Couldn't get guild")?;
    let channels = guild.channels(ctx).await?;

    let number_string = number.to_string();
    for (_id, channel) in channels {
        if channel.name.contains(&number_string) {
            ctx.say("Category/channels already seem to exist!").await?;
            return Ok(());
        }
    }

    // This is a really horrific way to just grab a random value to pre-initialize
    // We need the loop because the everyone ID is the same as the guild ID
    let everyone = RoleId::from(*guild.id.as_u64());

    let role = guild
        .create_role(ctx, |r| r.hoist(true).name(format!("CS {}", number_string)))
        .await
        .context("Couldn't create role")?;

    let category = guild
        .create_channel(ctx, |c| {
            c.name(format!("CS {}", number_string))
                .kind(ChannelType::Category)
                .permissions(vec![
                    PermissionOverwrite {
                        allow: Permissions::VIEW_CHANNEL,
                        deny: Permissions::empty(),
                        kind: PermissionOverwriteType::Role(role.id),
                    },
                    PermissionOverwrite {
                        allow: Permissions::empty(),
                        deny: Permissions::all(),
                        kind: PermissionOverwriteType::Role(everyone),
                    },
                ])
        })
        .await
        .context("Couldn't create category")?;

    guild
        .create_channel(ctx, |c| {
            c.name(format!("{}-resources", number_string))
                .kind(ChannelType::Text)
                .category(category.id)
        })
        .await
        .context("Couldn't create resources channel")?;

    guild
        .create_channel(ctx, |c| {
            c.name(format!("{}-general", number_string))
                .kind(ChannelType::Text)
                .category(category.id)
        })
        .await
        .context("Couldn't create general channel")?;

    guild
        .create_channel(ctx, |c| {
            c.name(format!("{}-assignment-discussion", number_string))
                .kind(ChannelType::Text)
                .category(category.id)
        })
        .await
        .context("Couldn't create assignment discussion channel")?;

    ctx.say("Success!").await?;
    Ok(())
}

use crate::types::{Context, Error};

use chrono::Duration;
use poise::serenity_prelude as serenity;
use serenity::{ChannelType, PermissionOverwrite, PermissionOverwriteType, Permissions};

#[poise::command(slash_command)]
pub async fn change_text_detect_cooldown(
    ctx: Context<'_>,
    #[description = "The cooldown in minutes"] cooldown: i64,
) -> Result<(), Error> {
    {
        ctx.data()
            .config
            .update_cooldown(Duration::minutes(cooldown));
    }
    ctx.say("Done!").await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn reload_config(ctx: Context<'_>) -> Result<(), Error> {
    ctx.data().reload();
    ctx.say("Successfully reloaded cooldown and responses from config.toml").await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn create_class_category(
    ctx: Context<'_>,
    #[description = "The class number, eg. for CS2420 put in \"2420\""] number: u32,
) -> Result<(), Error> {
    let guild = ctx.guild().unwrap();
    let channels = ctx.guild().unwrap().channels(ctx).await?;
    let roles = ctx.guild().unwrap().roles;

    let number_string = number.to_string();
    for (_id, channel) in channels {
        if channel.name.contains(&number_string) {
            ctx.say("Category/channels already seem to exist!").await?;
            return Ok(());
        }
    }

    // This is a really horrific way to just grab a random value to pre-initialize
    // We need the loop because the everyone ID is the same as the guild ID
    let mut everyone = roles.values().next().unwrap().clone();

    for (_id, role) in roles {
        if role.name.contains(&number_string) {
            ctx.say("Role already seems to exist!").await?;
            return Ok(());
        } else if role.id.as_u64() == guild.id.as_u64() {
            everyone = role;
        }
    }

    let role = guild
        .create_role(ctx, |r| r.hoist(true).name(format!("CS {}", number_string)))
        .await
        .unwrap();

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
                        kind: PermissionOverwriteType::Role(everyone.id),
                    },
                ])
        })
        .await
        .unwrap();
    guild
        .create_channel(ctx, |c| {
            c.name(format!("{}-resources", number_string))
                .kind(ChannelType::Text)
                .category(category.id)
        })
        .await
        .unwrap();
    guild
        .create_channel(ctx, |c| {
            c.name(format!("{}-general", number_string))
                .kind(ChannelType::Text)
                .category(category.id)
        })
        .await
        .unwrap();
    guild
        .create_channel(ctx, |c| {
            c.name(format!("{}-assignment-discussion", number_string))
                .kind(ChannelType::Text)
                .category(category.id)
        })
        .await
        .unwrap();

    ctx.say("Success!").await?;
    Ok(())
}

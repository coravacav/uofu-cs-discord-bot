use crate::types;
use chrono::Duration;
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::{PermissionOverwrite, PermissionOverwriteType, Permissions};
use serenity::ChannelType;

use types::{Context, Error};

use rand::prelude::*;

#[poise::command(slash_command)]
pub async fn change_text_detect_cooldown(
    ctx: Context<'_>,
    #[description = "The cooldown in minutes"] cooldown: i64,
) -> Result<(), Error> {
    {
        let mut text_detect_cooldown = ctx
            .data()
            .text_detect_cooldown
            .lock()
            .expect("Could not lock mutex");
        *text_detect_cooldown = Duration::minutes(cooldown);
    }
    ctx.say("Done!").await?;
    Ok(())
}

#[poise::command(slash_command)]
pub async fn create_class_category(
    ctx: Context<'_>,
    #[description = "The class number, eg. for CS2420 put in \"2420\""] number: u32,
) -> Result<(), Error> {
    let guild = ctx.guild().unwrap();
    let channels = ctx.guild().unwrap().channels(ctx).await?;

    let number_string = number.to_string();
    for (_id, channel) in channels {
        if channel.name.contains(&number_string) {
            ctx.say("Category already seems to exist!").await?;
            return Ok(());
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
                .permissions(vec![PermissionOverwrite {
                    allow: Permissions::VIEW_CHANNEL,
                    deny: Permissions::empty(),
                    kind: PermissionOverwriteType::Role(role.id),
                }])
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

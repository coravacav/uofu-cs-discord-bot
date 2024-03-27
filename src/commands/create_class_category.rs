use crate::data::PoiseContext;
use color_eyre::eyre::{Context, OptionExt, Result};
use poise::serenity_prelude::{self as serenity};
use serenity::{ChannelType, PermissionOverwrite, PermissionOverwriteType, Permissions};

#[poise::command(slash_command, required_permissions = "MANAGE_CHANNELS")]
pub async fn create_class_category(
    ctx: PoiseContext<'_>,
    #[description = "The class number, eg. for CS2420 put in \"2420\""] number: u32,
) -> Result<()> {
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.clone();
    let channels = guild.channels(ctx).await?;

    let number_string = number.to_string();
    for (_id, channel) in channels {
        if channel.name.contains(&number_string) {
            ctx.say("Category/channels already seem to exist!").await?;
            return Ok(());
        }
    }

    let role = guild
        .create_role(
            ctx,
            serenity::EditRole::new()
                .hoist(true)
                .name(format!("CS {}", number_string)),
        )
        .await
        .context("Couldn't create role")?;

    let category = guild
        .create_channel(
            ctx,
            serenity::CreateChannel::new(format!("CS {}", number_string))
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
                        kind: PermissionOverwriteType::Role(guild.id.everyone_role()),
                    },
                ]),
        )
        .await
        .context("Couldn't create category")?;

    guild
        .create_channel(
            ctx,
            serenity::CreateChannel::new(format!("{}-resources", number_string))
                .kind(ChannelType::Text)
                .category(category.id),
        )
        .await
        .context("Couldn't create resources channel")?;

    guild
        .create_channel(
            ctx,
            serenity::CreateChannel::new(format!("{}-general", number_string))
                .kind(ChannelType::Text)
                .category(category.id),
        )
        .await
        .context("Couldn't create general channel")?;

    guild
        .create_channel(
            ctx,
            serenity::CreateChannel::new(format!("{}-assignment-discussion", number_string))
                .kind(ChannelType::Text)
                .category(category.id),
        )
        .await
        .context("Couldn't create assignment discussion channel")?;

    ctx.say("Success!").await?;
    Ok(())
}

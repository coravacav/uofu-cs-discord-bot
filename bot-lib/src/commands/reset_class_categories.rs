use crate::commands::{get_channels, get_role};
use crate::data::PoiseContext;
use color_eyre::eyre::{OptionExt, Result, WrapErr};
use poise::serenity_prelude::{self as serenity};
use regex::Regex;
use serenity::ChannelType;

pub async fn reset_class_category_backend(ctx: PoiseContext<'_>, number: u32) -> Result<()> {
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let members = guild.members(ctx, None, None).await?;

    let general_channel_name = format!("{}-general", number);
    let gotten_channels = get_channels(ctx, guild, Regex::new(&general_channel_name)?).await?;
    let general_channel = gotten_channels
        .first()
        .ok_or_eyre("Could not find general channel!")?;

    let role_id = get_role(ctx, number).await?;

    let category_id = general_channel
        .parent_id
        .ok_or_eyre("Couldn't get category ID!")?;

    general_channel.delete(ctx).await?;

    guild
        .create_channel(
            ctx,
            serenity::CreateChannel::new(general_channel_name)
                .kind(ChannelType::Text)
                .category(category_id),
        )
        .await
        .wrap_err("Couldn't create general channel")?;

    let members_with_role = members
        .iter()
        .filter(|member| member.roles.contains(&role_id));

    for member in members_with_role {
        member.remove_role(ctx, role_id).await?;
    }

    Ok(())
}

#[poise::command(
    slash_command,
    required_permissions = "MANAGE_CHANNELS",
    description_localized(
        "en-US",
        "Resets a class category (clears the general channel, removes the role from everyone)"
    )
)]
pub async fn reset_class_category(
    ctx: PoiseContext<'_>,
    #[description = "The class number, eg. for CS2420 put in \"2420\""] number: u32,
) -> Result<()> {
    reset_class_category_backend(ctx, number).await?;
    ctx.say("Success!").await?;
    Ok(())
}

#[poise::command(
    slash_command,
    required_permissions = "MANAGE_CHANNELS",
    description_localized("en-US", "Resets all class categories")
)]
pub async fn reset_class_categories(ctx: PoiseContext<'_>) -> Result<()> {
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let removed_categories = get_channels(ctx, guild, Regex::new(r"\d{4}-general").unwrap())
        .await?
        .into_iter()
        .map(|channel| {
            channel
                .name
                .get(0..4)
                .unwrap_or("Intentional parse error")
                .parse::<u32>()
                .context("Parse error")
        });

    for category in removed_categories {
        reset_class_category_backend(ctx, category?).await?;
    }

    ctx.say("Success!").await?;

    Ok(())
}

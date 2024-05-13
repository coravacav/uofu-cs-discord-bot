use crate::data::PoiseContext;
use color_eyre::eyre::{OptionExt, Result, WrapErr};
use poise::serenity_prelude::{self as serenity};
use regex::Regex;
use serenity::{ChannelType};

pub async fn reset_class_category_backend(ctx: PoiseContext<'_>, number: u32) -> Result<()> {
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let channels = guild.channels(ctx).await?;
    let roles = guild.roles(ctx).await?;
    let members = guild.members(ctx, None, None).await?;
    let number_string = number.to_string();

    let general_channel_name = format!("{}-general", &number_string);
    let Some((_general_channel_id, general_channel)) = channels
        .iter()
        .find(|x| x.1.name.contains(&general_channel_name))
    else {
        ctx.say("Couldn't find the general channel!").await?;
        return Ok(());
    };

    let role_name = format!("CS {}", number_string);
    let Some((role_id, _role)) = roles.iter().find(|x| x.1.name.contains(&role_name)) else {
        ctx.say("Couldn't find the role!").await?;
        return Ok(());
    };

    let category_id = general_channel
        .parent_id
        .expect("Couldn't get category ID!");

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

    let memebrs_with_role = members
        .iter()
        .filter(|member| member.roles.contains(role_id));

    for member in memebrs_with_role {
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
    let channels = guild.channels(ctx).await?;

    let general_channel_pattern = Regex::new(r"\d{4}-general").unwrap();

    let removed_categories = channels
        .iter()
        .map(|channel| (&channel.1.name).to_string())
        .filter(|name| general_channel_pattern.is_match(name))
        .map(|name| {
            name[0..4]
                .parse::<u32>()
                .expect("Parse error on class category name")
        })
        .collect::<Vec<u32>>();

    for category in removed_categories {
        reset_class_category_backend(ctx, category).await?;
    }

    ctx.say("Success!").await?;

    Ok(())
}

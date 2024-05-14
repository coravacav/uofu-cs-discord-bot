use crate::commands::{get_channels, get_role};
use crate::data::PoiseContext;
use color_eyre::eyre::{OptionExt, Result};
use regex::Regex;

#[poise::command(
    slash_command,
    required_permissions = "MANAGE_CHANNELS",
    description_localized("en-US", "Deletes a class category")
)]
pub async fn delete_class_category(
    ctx: PoiseContext<'_>,
    #[description = "The class number, eg. for CS2420 put in \"2420\""] number: u32,
) -> Result<()> {
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let channels = guild.channels(ctx).await?;
    let number_string = number.to_string();

    let category_and_role_name = format!("CS {}", &number_string);
    let category_channel = &get_channels(
        ctx,
        guild,
        Regex::new(&format!("^{}$", category_and_role_name))?,
    )
    .await?[0];

    let children_channels = channels.iter().filter(|x| match x.1.parent_id {
        Some(parent) => parent.eq(&category_channel.id),
        None => false,
    });

    let role_id = get_role(ctx, number).await?;

    category_channel.delete(ctx).await?;
    for channel in children_channels {
        channel.1.delete(ctx).await?;
    }
    guild.delete_role(ctx, role_id).await?;

    ctx.say("Success!").await?;
    Ok(())
}

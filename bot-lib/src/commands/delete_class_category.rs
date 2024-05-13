use crate::data::PoiseContext;
use color_eyre::eyre::{OptionExt, Result};

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
    let roles = guild.roles(ctx).await?;
    let number_string = number.to_string();

    let category_and_role_name = format!("CS {}", &number_string);
    let Some((category_channel_id, category_channel)) = channels
        .iter()
        .find(|x| x.1.name.contains(&category_and_role_name))
        else {
            ctx.say("Couldn't find the category!").await?;
            return Ok(());
        };

    let children_channels = channels.iter().filter(|x| {
        match x.1.parent_id {
            Some(parent) => parent.eq(category_channel_id),
            None => false,
        }
    });

    let Some((role_id, _)) = roles.iter().find(|x| x.1.name.contains(&category_and_role_name)) else {
        ctx.say("Couldn't find the role!").await?;
        return Ok(());
    };

    category_channel.delete(ctx).await?;
    for channel in children_channels {
        channel.1.delete(ctx).await?;
    }
    guild.delete_role(ctx, role_id).await?;


    ctx.say("Success!").await?;
    Ok(())
}

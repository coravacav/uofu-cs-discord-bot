pub mod class_roles;
pub mod course_catalog;
pub mod create_class_category;
pub mod delete_class_category;
pub mod help;
pub mod lynch;
pub mod register;
pub mod reset_class_categories;
pub mod sathya;
pub mod set_bot_role;
pub mod set_dog_role;
pub mod timeout;

use crate::data::PoiseContext;
use color_eyre::eyre::{OptionExt, Result};
use color_eyre::Report;
use poise::serenity_prelude::{GuildChannel, GuildId, Member, RoleId};
use regex::Regex;

/// Finds all channels in the given guild, where the name matches the given regex
pub async fn get_channels(
    ctx: PoiseContext<'_>,
    guild: GuildId,
    pattern: Regex,
) -> Result<Vec<GuildChannel>> {
    let channels = guild.channels(ctx).await?;

    let filtered_channels = channels
        .values()
        .filter(|channel| pattern.is_match(&channel.name))
        .map(|x| x.to_owned())
        .collect();

    Ok(filtered_channels)
}

pub async fn get_role(ctx: PoiseContext<'_>, number: u32) -> Result<RoleId> {
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let roles = guild.roles(ctx).await?;

    let role_name = format!("CS {}", number);
    let Some(role_id) = roles
        .iter()
        .find_map(|(role_id, role)| role.name.contains(&role_name).then_some(*role_id))
    else {
        ctx.say("Couldn't find the class!").await?;
        return Err(Report::msg("Class role not found"));
    };

    Ok(role_id)
}

pub async fn get_author(ctx: PoiseContext<'_>) -> Result<Member> {
    let author = ctx.author();
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;

    Ok(guild.member(ctx, author.id).await?)
}

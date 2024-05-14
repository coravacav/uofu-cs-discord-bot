pub mod add_bot_role;
pub mod course_catalog;
pub mod create_class_category;
pub mod help;
pub mod lynch;
pub mod register;
pub mod remove_bot_role;
pub mod timeout;
pub mod reset_class_categories;
pub mod delete_class_category;
pub mod class_roles;

use color_eyre::eyre::Result;
use poise::serenity_prelude::{GuildChannel, GuildId};
use regex::Regex;
use crate::data::PoiseContext;

/// Finds all channels in the given guild, where the name matches the given regex
pub async fn find_channels(ctx: PoiseContext<'_>, guild: GuildId, pattern: Regex) -> Result<Vec<GuildChannel>>{
    let channels = guild.channels(ctx).await?;

    let filtered_channels = channels
        .iter()
        .filter(|channel| pattern.is_match(&channel.1.name))
        .map(|x| x.1.to_owned())
        .collect::<Vec<GuildChannel>>();

    Ok(filtered_channels)
}
pub mod aur_search;
pub mod class_commands;
pub mod course_catalog;
pub mod help;
pub mod llm_prompt;
pub mod register;
pub mod sathya;
pub mod set_bot_role;
pub mod set_dog_role;
pub mod timeout;
pub mod yeet;

use crate::data::PoiseContext;
use color_eyre::eyre::{OptionExt, Result};
use poise::serenity_prelude::Member;

pub async fn get_member(ctx: PoiseContext<'_>) -> Result<Member> {
    let author = ctx.author();
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;

    Ok(guild.member(ctx, author.id).await?)
}

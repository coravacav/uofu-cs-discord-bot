mod admin;
mod anon_notify;
mod aur_search;
mod bank;
mod cice;
mod class_commands;
mod clip_that;
mod course_catalog;
mod db_admin;
mod extract;
mod feedback;
mod help;
mod message_limit;
mod mod_abuse;
mod reroll_reply;
mod sathya;
mod set_bot_role;
mod set_dog_role;
mod timeout;
mod track_flight;
mod yeet;

pub use admin::*;
pub use anon_notify::*;
pub use aur_search::*;
pub use bank::*;
pub use cice::*;
pub use class_commands::*;
pub use clip_that::*;
pub use course_catalog::*;
pub use db_admin::*;
pub use extract::*;
pub use feedback::*;
pub use help::*;
pub use message_limit::*;
pub use mod_abuse::*;
pub use reroll_reply::*;
pub use sathya::*;
pub use set_bot_role::*;
pub use set_dog_role::*;
pub use timeout::*;
pub use track_flight::*;
pub use yeet::*;

use crate::data::PoiseContext;
use color_eyre::eyre::{Context, ContextCompat, OptionExt, Result};
use poise::serenity_prelude::{CreateMessage, EditMember, Member, Mentionable};
use std::time::Duration;

pub async fn get_member(ctx: PoiseContext<'_>) -> Result<Member> {
    let author = ctx.author();
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;

    Ok(guild.member(ctx, author.id).await?)
}

pub async fn is_stefan(ctx: PoiseContext<'_>) -> Result<bool> {
    let author = ctx.author();
    let channel_id = ctx.channel_id();
    let guild_id = ctx.guild_id().wrap_err("No guild ID?")?;

    if author.id == 216767618923757568 {
        return Ok(true);
    }

    let timeout_end = chrono::Utc::now() + Duration::from_secs(300);

    if guild_id
        .edit_member(
            ctx,
            author.id,
            EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
        )
        .await
        .wrap_err("Failed to edit member")
        .is_err()
    {
        ctx.say("You're a mod why are you trying this? How dare you. You should know better.")
            .await?;
        return Ok(false);
    };

    channel_id
        .send_message(
            &ctx,
            CreateMessage::new().content(format!(
                "{} dared to impersonate Stefan, they were timed out for 5 minutes",
                author.mention()
            )),
        )
        .await?;

    ctx.say("PAH!").await?;

    Ok(false)
}

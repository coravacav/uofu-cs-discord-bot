pub mod aur_search;
pub mod bank;
pub mod class_commands;
pub mod course_catalog;
pub mod db_admin;
pub mod feedback;
pub mod help;
pub mod llm_prompt;
pub mod register;
pub mod russian_roulette;
pub mod sathya;
pub mod set_bot_role;
pub mod set_dog_role;
pub mod timeout;
pub mod yeet;

use std::time::Duration;

use crate::data::PoiseContext;
use color_eyre::eyre::{Context, ContextCompat, OptionExt, Result};
use poise::serenity_prelude::{self as serenity, CreateMessage, Member, Mentionable};

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
            serenity::EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
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

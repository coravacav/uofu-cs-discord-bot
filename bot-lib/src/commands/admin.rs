use crate::{commands::is_stefan, data::PoiseContext};
use color_eyre::eyre::{Context, OptionExt, Result};
use poise::serenity_prelude::{EditMember, UserId};

#[poise::command(
    slash_command,
    subcommands(
        "remove_timeout",
    ),
    guild_only,
    check = is_stefan
)]
pub async fn admin(_ctx: PoiseContext<'_>) -> Result<()> {
    Ok(())
}

#[poise::command(slash_command, ephemeral = true)]
pub async fn remove_timeout(ctx: PoiseContext<'_>, user: UserId) -> Result<()> {
    let guild_id = ctx.guild_id().ok_or_eyre("No guild ID?")?;

    let Ok(_) = guild_id
        .edit_member(
            ctx,
            user,
            EditMember::new().disable_communication_until(chrono::Utc::now().to_rfc3339()),
        )
        .await
        .wrap_err("Failed to edit member")
    else {
        ctx.say("Failed to remove timeout").await?;
        return Ok(());
    };

    Ok(())
}

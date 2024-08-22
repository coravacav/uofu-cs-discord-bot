use crate::data::PoiseContext;
use color_eyre::eyre::{ContextCompat, OptionExt, Result, WrapErr};
use poise::serenity_prelude::{EditMember, GuildId, MessageBuilder, User, UserId};

const SATHYA_USER_ID: UserId = UserId::new(444895960577998860);

#[poise::command(
    slash_command,
    prefix_command,
    description_localized("en-US", "Nickname someone \"Sathya\" (only if you are Sathya!)")
)]
pub async fn sathya(ctx: PoiseContext<'_>, victim: User) -> Result<()> {
    let author = ctx.author();
    let guild_id = ctx.guild_id().wrap_err("No guild ID?")?;

    if author.id != SATHYA_USER_ID {
        ctx.say("You are not Sathya! How dare you.").await?;
        return Ok(());
    }

    let author_has_role = author
        .has_role(
            ctx,
            guild_id,
            ctx.data().config.read().await.bot_react_role_id,
        )
        .await?;

    if !author_has_role {
        ctx.say("Target doesn't have bot react role!").await?;
        return Ok(());
    }

    let guild: GuildId = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;

    if let Err(err) = guild
        .member(ctx, victim.id)
        .await
        .wrap_err("Couldn't get member")?
        .edit(ctx, EditMember::new().nickname("Sathya"))
        .await
        .wrap_err("Couldn't apply nickname")
    {
        ctx.say("Couldn't apply nickname, you're probably targeting someone too powerful.")
            .await?;

        return Err(err);
    }

    ctx.say(
        MessageBuilder::new()
            .push("Sathya sathya'd ")
            .mention(&victim)
            .push(" \"Sathya\"")
            .build(),
    )
    .await
    .wrap_err("Couldn't send message")?;

    Ok(())
}

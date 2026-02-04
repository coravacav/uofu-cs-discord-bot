use crate::{
    data::PoiseContext,
    utils::{GetRelativeTimestamp, SendReplyEphemeral},
};
use color_eyre::eyre::{ContextCompat, Result};
use human_repr::HumanDuration;
use humantime::parse_duration;
use poise::{
    CreateReply,
    serenity_prelude::{EditMember, Mentionable, User},
};
use rand::prelude::*;
use std::time::Duration;

#[poise::command(slash_command, required_permissions = "MODERATE_MEMBERS", guild_only)]
pub async fn mod_abuse(
    ctx: PoiseContext<'_>,
    #[description = "Target of abuse"] target: User,
    #[description = "The amount of time to time target out, like '1h' or '3m'"] time_text: String,
    #[description = "Optional reason for the timeout"] reason: Option<String>,
) -> Result<()> {
    if time_text.len() > 20 {
        return ctx
            .reply_ephemeral("Send something reasonable, please.")
            .await;
    }

    let Ok(time) = parse_duration(&time_text) else {
        return ctx
            .reply_ephemeral("Invalid time format! Say something like '1h' or '3m'")
            .await;
    };

    if time > Duration::from_secs(60 * 60 * 24 * 28) {
        return ctx
            .reply_ephemeral(
                "Discord isn't cool and doesn't let you time out for more than 28 days",
            )
            .await;
    }

    if time < Duration::from_secs(1) {
        return ctx.reply_ephemeral("_huh_").await;
    }

    let timeout_end = chrono::Utc::now() + time;

    let guild_id = ctx.guild_id().wrap_err("No guild ID?")?;

    if guild_id
        .edit_member(
            ctx,
            target.id,
            EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
        )
        .await
        .is_err()
    {
        ctx.say(format!("Everyone laugh at {} with timeout privileges that dared to abuse them (to attempt to kill {}) and then STILL FAIL.", ctx.author().mention(), target.mention())).await?;

        return Ok(());
    };

    let reason = reason
        .map(|r| format!(" because \"{r}\""))
        .unwrap_or_default();

    let participate_verbiage: [&str; 2] = [
        (
            ["sentenced to not participate", "was previously timed out"],
            9,
        ),
        (
            [
                "banished to the shadow realm",
                "took a vacation in the shadow realm",
            ],
            1,
        ),
    ]
    .choose_weighted(&mut rand::rng(), |choice| choice.1)
    .unwrap()
    .0;

    let reply_handle = ctx
        .say(format!(
            "❗mod abuse alert❗\n\n{} has been {} by {}{}.\n\nThey will return {}.",
            target.mention(),
            participate_verbiage[0],
            ctx.author().mention(),
            reason,
            timeout_end.discord_relative_timestamp(),
        ))
        .await?;

    tokio::time::sleep(time - std::time::Duration::from_secs(1)).await;

    reply_handle
        .edit(
            ctx,
            CreateReply::default().content(format!(
                "❗mod abuse alert❗\n\n{} {} for {} by {}{}.\n\nThey have since returned.",
                target.mention(),
                participate_verbiage[1],
                time.human_duration(),
                ctx.author().mention(),
                reason
            )),
        )
        .await?;

    Ok(())
}

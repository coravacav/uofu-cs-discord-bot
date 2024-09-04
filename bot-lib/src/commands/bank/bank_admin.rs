use super::build_history_message;
use crate::data::PoiseContext;
use crate::{commands::is_stefan, SayThenDelete};
use bot_db::bank::BankDb;
use color_eyre::eyre::Result;
use poise::serenity_prelude::{Mentionable, User};

#[poise::command(
    slash_command,
    subcommands(
        "give_charity",
        "inspect_history",
        "inspect_balance",
        "global_rankings",
    )
)]
pub async fn bank_admin(_ctx: PoiseContext<'_>) -> Result<()> {
    Ok(())
}

/// For Stefan only, give charity.
#[poise::command(
    slash_command,
    ephemeral = true,
    hide_in_help = true,
    check = is_stefan)]
pub async fn give_charity(
    ctx: PoiseContext<'_>,
    charity_recipient: User,
    amount: i64,
) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;

    bank.change(
        charity_recipient.id,
        amount,
        String::from("Stefan is very generous"),
    )?;

    ctx.say(format!(
        "{} has their balance updated to {}",
        charity_recipient.mention(),
        bank.get(charity_recipient.id)?.balance
    ))
    .await?;

    Ok(())
}

/// See the last 20 transactions for a user
#[poise::command(slash_command, ephemeral = true)]
pub async fn inspect_history(ctx: PoiseContext<'_>, user: User) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;
    let user_id = user.id;

    let Some(history) = bank.get_history(user_id)? else {
        ctx.say("No history found for that user").await?;
        return Ok(());
    };

    ctx.say_then_delete(build_history_message(history, user_id))
        .await?;

    Ok(())
}

/// See a user's balance
#[poise::command(slash_command, ephemeral = true)]
pub async fn inspect_balance(ctx: PoiseContext<'_>, user: User) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;
    let user_id = user.id;
    let account = bank.get(user_id)?;

    ctx.say_then_delete(format!(
        "{}'s balance is {}",
        user.mention(),
        account.balance
    ))
    .await?;

    Ok(())
}

/// For stefan only, see the global rankings
#[poise::command(slash_command, ephemeral = true, check = is_stefan)]
pub async fn global_rankings(ctx: PoiseContext<'_>) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;

    let rankings = bank.get_global_rankings()?;
    let mut message_text = String::from("### Global Rankings:\n");

    for (user_id, account) in rankings {
        message_text.push_str(&format!("{}: {}, ", user_id.mention(), account.balance));
    }

    ctx.say_then_delete(message_text).await?;

    Ok(())
}

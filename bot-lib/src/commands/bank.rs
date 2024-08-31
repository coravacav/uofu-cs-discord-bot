use std::{
    sync::LazyLock,
    time::{Duration, Instant},
};

use crate::commands::is_stefan;
use crate::data::PoiseContext;
use bot_db::bank::BankDb;
use color_eyre::eyre::Result;
use dashmap::DashMap;
use poise::serenity_prelude::{Mentionable, User, UserId};
use rand::Rng;

#[poise::command(
    slash_command,
    subcommands("balance", "income", "give_charity", "gamble")
)]
pub async fn bank(_ctx: PoiseContext<'_>) -> Result<()> {
    Ok(())
}

/// What's my balance?
#[poise::command(slash_command, ephemeral = true)]
pub async fn balance(ctx: PoiseContext<'_>) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;

    ctx.say(format!(
        "Your balance is {}",
        bank.get(ctx.author().id)?.balance
    ))
    .await?;

    Ok(())
}

static INSTANT_BY_USER_ID: LazyLock<DashMap<UserId, Instant>> = LazyLock::new(DashMap::new);
const PER_USER_INCOME_TIMEOUT_SECONDS: u64 = 60;

/// Get some income (5 coins, you can do it once per minute)
#[poise::command(slash_command, ephemeral = true)]
pub async fn income(ctx: PoiseContext<'_>) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;
    let user_id = ctx.author().id;

    match INSTANT_BY_USER_ID.get(&user_id) {
        Some(last_time)
            if last_time.elapsed() < Duration::from_secs(PER_USER_INCOME_TIMEOUT_SECONDS) =>
        {
            ctx.reply("Federal law requires you calm down").await?;
            return Ok(());
        }
        _ => {}
    }

    INSTANT_BY_USER_ID.insert(user_id, Instant::now());

    bank.change(user_id, 5, String::from("Income"))?;

    ctx.reply(format!(
        "Paycheck deposited! Your new balance is {}",
        bank.get(user_id)?.balance
    ))
    .await?;

    Ok(())
}

/// For Stefan only, give charity.
#[poise::command(slash_command, ephemeral = true, check=is_stefan, hide_in_help = true)]
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

/// Gamble. KingFisher skims 1 coin no matter what.
#[poise::command(slash_command, ephemeral = true)]
pub async fn gamble(
    ctx: PoiseContext<'_>,
    #[description = "Amount to gamble"] amount: i64,
    #[description = "Odds of success, between 0 and 1"] odds_of_success: f64,
) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;
    let user_id = ctx.author().id;

    if amount <= 0 {
        ctx.say("Trying to gamble negative money? No.").await?;
        return Ok(());
    }

    if !(0.0..=1.0).contains(&odds_of_success) {
        ctx.say("Gambling is only allowed for the literate").await?;
        return Ok(());
    }

    let balance = bank.get(user_id)?.balance;
    if balance < amount {
        ctx.say(format!(
            "You don't have enough money! You have ${}",
            balance
        ))
        .await?;
        return Ok(());
    }

    let winnings = (amount as f64 / odds_of_success).round() as i64 - 1 - amount;
    let success = rand::thread_rng().gen_bool(odds_of_success);

    let change = if success { winnings } else { -amount };

    let Some(account) = bank.change(
        user_id,
        change,
        format!("Gamble for {} at odds {}", amount, odds_of_success),
    )?
    else {
        tracing::error!("How did we get no account back?");
        ctx.say("Something went wrong, contact Stefan").await?;
        return Ok(());
    };

    ctx.say(format!(
        "You {}! Your new balance is {}",
        if success { "won" } else { "lost" },
        account.balance
    ))
    .await?;

    Ok(())
}

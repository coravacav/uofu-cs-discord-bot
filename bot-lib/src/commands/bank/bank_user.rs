use super::build_history_message;
use crate::{data::PoiseContext, SayThenDelete};
use bot_db::bank::BankDb;
use color_eyre::eyre::Result;
use dashmap::DashMap;
use poise::serenity_prelude::UserId;
use rand::Rng;
use std::{
    sync::LazyLock,
    time::{Duration, Instant},
};

#[poise::command(slash_command, subcommands("balance", "income", "gamble", "history"))]
pub async fn bank(_ctx: PoiseContext<'_>) -> Result<()> {
    Ok(())
}

/// What's my balance?
#[poise::command(slash_command, ephemeral = true)]
pub async fn balance(ctx: PoiseContext<'_>) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;

    ctx.say_then_delete(format!(
        "Your balance is {}",
        bank.get(ctx.author().id)?.balance
    ))
    .await?;

    Ok(())
}

static INSTANT_BY_USER_ID: LazyLock<DashMap<UserId, Instant>> = LazyLock::new(DashMap::new);
const PER_USER_INCOME_TIMEOUT_SECONDS: u64 = 60;
static BONUS_BY_USER_ID: LazyLock<DashMap<UserId, i64>> = LazyLock::new(DashMap::new);

pub fn reset_user_bonus(user_id: UserId) {
    if let Some(entry) = BONUS_BY_USER_ID.try_entry(user_id) {
        *entry.or_insert(-1) = -1;
    }
}

pub fn get_user_bonus(user_id: UserId) -> i64 {
    match BONUS_BY_USER_ID.try_entry(user_id) {
        Some(entry) => {
            let mut entry = entry.or_insert(-1);
            *entry += 1;
            *entry
        }
        None => 0,
    }
}

/// Get some income (5 coins, once per minute, bonus if you repeat without gambling)
#[poise::command(slash_command, ephemeral = true)]
pub async fn income(ctx: PoiseContext<'_>) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;
    let user_id = ctx.author().id;

    match INSTANT_BY_USER_ID.get(&user_id) {
        Some(last_time)
            if last_time.elapsed() < Duration::from_secs(PER_USER_INCOME_TIMEOUT_SECONDS) =>
        {
            ctx.say_then_delete("Federal law requires you calm down")
                .await?;
            return Ok(());
        }
        _ => {}
    }

    INSTANT_BY_USER_ID.insert(user_id, Instant::now());
    let bonus_amount = get_user_bonus(user_id);

    bank.change(user_id, 5 + bonus_amount, String::from("Income"))?;

    ctx.say_then_delete(format!(
        "Paycheck deposited{}! Your new balance is {}",
        if bonus_amount > 0 {
            format!(" with a bonus of {}", bonus_amount)
        } else {
            String::new()
        },
        bank.get(user_id)?.balance
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

    reset_user_bonus(user_id);

    if amount <= 0 {
        ctx.say_then_delete("Trying to gamble negative money? No.")
            .await?;
        return Ok(());
    }

    if !(0.0..=1.0).contains(&odds_of_success) {
        ctx.say_then_delete("Gambling is only allowed for the literate")
            .await?;
        return Ok(());
    }

    let balance = bank.get(user_id)?.balance;
    if balance < amount {
        ctx.say_then_delete(format!(
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
        ctx.say_then_delete("Something went wrong, contact Stefan")
            .await?;
        return Ok(());
    };

    ctx.say_then_delete(format!(
        "You {}! Your new balance is {}",
        if success { "won" } else { "lost" },
        account.balance
    ))
    .await?;

    Ok(())
}

/// Inspect your own history
#[poise::command(slash_command, ephemeral = true)]
pub async fn history(ctx: PoiseContext<'_>) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;
    let author = ctx.author().id;

    let Some(history) = bank.get_history(author)? else {
        ctx.say_then_delete("No history found for that user")
            .await?;
        return Ok(());
    };

    ctx.say_then_delete(build_history_message(history, author))
        .await?;

    Ok(())
}

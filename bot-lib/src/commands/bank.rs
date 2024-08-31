use std::{
    sync::LazyLock,
    time::{Duration, Instant},
};

use crate::commands::is_stefan;
use crate::data::PoiseContext;
use bot_db::bank::{BankDb, Change};
use color_eyre::eyre::Result;
use dashmap::DashMap;
use poise::serenity_prelude::{Mentionable, User, UserId};
use rand::Rng;

#[poise::command(slash_command, subcommands("balance", "income", "gamble", "history",))]
pub async fn bank(_ctx: PoiseContext<'_>) -> Result<()> {
    Ok(())
}

#[poise::command(
    slash_command,
    subcommands(
        "give_charity",
        "inspect_history",
        "inspect_balance"
    ),
    check = is_stefan
)]
pub async fn bank_admin(_ctx: PoiseContext<'_>) -> Result<()> {
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

/// For stefan only, see a user's balance
#[poise::command(slash_command, ephemeral = true, check = is_stefan)]
pub async fn inspect_balance(ctx: PoiseContext<'_>, user: User) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;
    let user_id = user.id;
    let account = bank.get(user_id)?;

    ctx.say(format!(
        "{}'s balance is {}",
        user.mention(),
        account.balance
    ))
    .await?;

    Ok(())
}

static INSTANT_BY_USER_ID: LazyLock<DashMap<UserId, Instant>> = LazyLock::new(DashMap::new);
const PER_USER_INCOME_TIMEOUT_SECONDS: u64 = 60;
static BONUS_BY_USER_ID: LazyLock<DashMap<UserId, i64>> = LazyLock::new(DashMap::new);

pub fn reset_user_bonus(user_id: UserId) {
    BONUS_BY_USER_ID.remove(&user_id);
}

/// Get some income (5 coins, once per minute, bonus if you repeat without sending messages)
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
    let mut bonus_amount = BONUS_BY_USER_ID.entry(user_id).or_insert(-1);
    *bonus_amount += 1;
    let bonus_amount = *bonus_amount;

    bank.change(user_id, 5 + bonus_amount, String::from("Income"))?;

    ctx.reply(format!(
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

/// For Stefan only, give charity.
#[poise::command(slash_command, ephemeral = true, hide_in_help = true)]
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

    reset_user_bonus(user_id);

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

fn build_history_message(history: impl DoubleEndedIterator<Item = Change>, user: UserId) -> String {
    let mut message_text = String::from("### History:\n");

    message_text.push_str(&user.mention().to_string());
    message_text.push('\n');

    for Change { amount, reason } in history.rev().take(20) {
        message_text.push_str(&format!("`{:>9}`: {}\n", amount, reason));
    }

    message_text
}

/// Inspect your own history
#[poise::command(slash_command, ephemeral = true)]
pub async fn history(ctx: PoiseContext<'_>) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;
    let author = ctx.author().id;

    let Some(history) = bank.get_history(author)? else {
        ctx.say("No history found for that user").await?;
        return Ok(());
    };

    ctx.say(build_history_message(history, author)).await?;

    Ok(())
}

/// For stefan only, see the history of a user
#[poise::command(slash_command, ephemeral = true, check = is_stefan)]
pub async fn inspect_history(ctx: PoiseContext<'_>, user: User) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;
    let user_id = user.id;

    let Some(history) = bank.get_history(user_id)? else {
        ctx.say("No history found for that user").await?;
        return Ok(());
    };

    ctx.say(build_history_message(history, user_id)).await?;

    Ok(())
}

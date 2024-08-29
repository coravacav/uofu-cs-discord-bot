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

/// What's my balance?
#[poise::command(slash_command, ephemeral = true)]
pub async fn fishercoin_balance(ctx: PoiseContext<'_>) -> Result<()> {
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
            ctx.reply("Please wait a minute before asking again")
                .await?;
            return Ok(());
        }
        _ => {}
    }

    INSTANT_BY_USER_ID.insert(user_id, Instant::now());

    bank.change(user_id, 5, String::from("Income"))?;

    ctx.reply(format!(
        "Good work! Your new balance is {}",
        bank.get(user_id)?.balance
    ))
    .await?;

    Ok(())
}

/// For Stefan only, give charity.
#[poise::command(slash_command, ephemeral = true, check=is_stefan)]
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

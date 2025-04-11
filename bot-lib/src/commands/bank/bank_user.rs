use super::build_history_message;
use crate::{SayThenDelete, data::PoiseContext};
use bot_db::bank::BankDb;
use color_eyre::eyre::Result;
use parking_lot::Mutex;
use poise::{
    CreateReply,
    serenity_prelude::{
        CreateActionRow, CreateButton, CreateInteractionResponse, CreateInteractionResponseMessage,
        UserId,
    },
};
use rand::Rng;
use std::{
    collections::HashMap,
    sync::LazyLock,
    time::{Duration, Instant},
};

#[poise::command(
    slash_command,
    subcommands("balance", "income", "gamble", "history", "casino")
)]
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

const PER_USER_INCOME_TIMEOUT_SECONDS: u64 = 60;

static INSTANT_BY_USER_ID: LazyLock<Mutex<HashMap<UserId, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static BONUS_BY_USER_ID: LazyLock<Mutex<HashMap<UserId, i64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn reset_user_bonus(user_id: UserId) {
    BONUS_BY_USER_ID.lock().remove(&user_id);
}

pub fn get_user_bonus(user_id: UserId) -> i64 {
    *BONUS_BY_USER_ID
        .lock()
        .entry(user_id)
        .and_modify(|bonus| *bonus += 1)
        .or_insert(0)
}

/// Get some income (5 coins, once per minute, bonus if you repeat without gambling)
#[poise::command(slash_command, ephemeral = true)]
pub async fn income(ctx: PoiseContext<'_>) -> Result<()> {
    let bank = BankDb::new(&ctx.data().db)?;
    let user_id = ctx.author().id;

    let old_time = INSTANT_BY_USER_ID.lock().insert(user_id, Instant::now());

    if let Some(last_time) = old_time {
        if last_time.elapsed() < Duration::from_secs(PER_USER_INCOME_TIMEOUT_SECONDS) {
            ctx.say_then_delete("Federal law requires you calm down")
                .await?;
            return Ok(());
        }
    }

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
    let success = rand::rng().random_bool(odds_of_success);

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

/// Start the casino games
#[poise::command(slash_command, ephemeral = true)]
pub async fn casino(ctx: PoiseContext<'_>) -> Result<()> {
    // create a menu to pick the game

    let msg = ctx
        .send(
            CreateReply::default()
                .content("Pick a game")
                .components(vec![CreateActionRow::Buttons(vec![
                    CreateButton::new("Test button").label("WOWIE"),
                ])]),
        )
        .await?;

    let msg = msg.into_message().await?;
    let interaction = msg
        .await_component_interaction(ctx)
        .timeout(Duration::from_secs(10))
        .await;

    match interaction {
        Some(interaction) => {
            interaction
                .create_response(
                    ctx,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content("Done")
                            .components(vec![CreateActionRow::Buttons(vec![
                                CreateButton::new("IT WORKED").label("IT WORKED"),
                            ])]),
                    ),
                )
                .await?;

            tokio::time::sleep(Duration::from_millis(500)).await;

            interaction.delete_response(ctx).await.ok();
        }
        _ => {
            msg.delete(ctx).await.ok();
            return Ok(());
        }
    }

    Ok(())
}

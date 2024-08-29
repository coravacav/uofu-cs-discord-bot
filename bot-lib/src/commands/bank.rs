use crate::commands::is_stefan;
use crate::data::PoiseContext;
use bot_db::bank::BankDb;
use color_eyre::eyre::Result;
use poise::serenity_prelude::{Mentionable, User};

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

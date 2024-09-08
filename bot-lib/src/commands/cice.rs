use crate::{data::PoiseContext, SayThenDelete};
use color_eyre::eyre::Result;
use poise::serenity_prelude::Mentionable;
use rand::Rng;

#[poise::command(slash_command, prefix_command, rename = "coinflip")]
pub async fn coinflip(ctx: PoiseContext<'_>, optional_explanation: Option<String>) -> Result<()> {
    let heads = rand::thread_rng().gen_bool(0.5);

    ctx.say_then_delete(format!(
        "{} flipped a coin and got {}{}",
        ctx.author().mention(),
        if heads { "heads" } else { "tails" },
        if let Some(reason) = optional_explanation {
            format!(" because \"{}\"", reason)
        } else {
            String::new()
        }
    ))
    .await?;

    Ok(())
}

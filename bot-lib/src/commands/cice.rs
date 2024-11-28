use crate::{SayThenDelete, data::PoiseContext};
use color_eyre::eyre::Result;
use poise::serenity_prelude::Mentionable;
use rand::Rng;

#[poise::command(slash_command, rename = "coinflip")]
pub async fn coinflip(ctx: PoiseContext<'_>, optional_explanation: Option<String>) -> Result<()> {
    let heads = rand::thread_rng().gen_bool(0.5);
    let lands_on_its_side = rand::thread_rng().gen_bool(0.001);

    if lands_on_its_side {
        ctx.say(format!(
            "{} flipped a coin and it LANDED ON ITS SIDE??? (0.1% chance){}",
            ctx.author().mention(),
            if let Some(reason) = optional_explanation {
                format!(" \"{}\"", reason)
            } else {
                String::new()
            }
        ))
        .await?;

        return Ok(());
    }

    ctx.say_then_delete(format!(
        "{} flipped a coin and got {}{}",
        ctx.author().mention(),
        if heads { "heads" } else { "tails" },
        if let Some(reason) = optional_explanation {
            format!(" \"{}\"", reason)
        } else {
            String::new()
        }
    ))
    .await?;

    Ok(())
}

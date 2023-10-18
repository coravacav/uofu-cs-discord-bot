use crate::types;
use poise::serenity_prelude as serenity;
use poise::Event;
use serenity::Message;

use types::Data;
use types::Error;

use rand::prelude::*;

pub async fn text_detection(
    ctx: &serenity::Context,
    event: &Event<'_>,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
    message: &Message,
) -> Result<(), Error> {
    if message.content.to_lowercase().contains("rust") && !message.author.bot {
        message
            .reply(ctx, rust_response())
            .await?;
    }

    return Ok(());
}

fn rust_response<'a>() -> &'a str {
    let i = random::<u8>() % 4;
    match i {
        1 => "RUST MENTIONED :crab: :crab: :crab:",
        2 => "<@237717840818470913>",
        3 => "Rust is simply the best programming language. Nothing else can compare. I am naming my kids Rust and Ferris.",
        _ => "Rust? Oh, you mean the game?"
    }
}
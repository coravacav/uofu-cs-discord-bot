use crate::types;
use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude as serenity;
use poise::Event;
use serenity::Message;
use std::sync::Mutex;

use types::{Data, Error};

use rand::prelude::*;

pub async fn text_detection(
    ctx: &serenity::Context,
    _event: &Event<'_>,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
    message: &Message,
) -> Result<(), Error> {
    if message.content.to_lowercase().contains("rust") && !message.author.bot {
        if cooldown_checker(
            &data.last_rust_response,
            &data.text_detect_cooldown,
            message.timestamp.with_timezone(&Utc),
        ) {
            message.reply(ctx, rust_response()).await?;
        }
    } else if message.content.to_lowercase().contains("tkinter") && !message.author.bot {
        if cooldown_checker(
            &data.last_tkinter_response,
            &data.text_detect_cooldown,
            message.timestamp.with_timezone(&Utc),
        ) {
            let file = [(
                &tokio::fs::File::open("./assets/tkinter.png").await?,
                "./assets/tkinter.png",
            )];
            message
                .channel_id
                .send_message(ctx, |m| {
                    m.reference_message(message);
                    m.content("TKINTER MENTIONED");
                    m.files(file);
                    return m;
                })
                .await?;
        }
    }

    Ok(())
}

/// Checks if the cooldown is met. If yes, it is, returns true and resets the cooldown. If not,
/// returns false and does nothing.
fn cooldown_checker(
    last_message: &Mutex<DateTime<Utc>>,
    cooldown: &Mutex<Duration>,
    timestamp: DateTime<Utc>,
) -> bool {
    let mut last_message = last_message.lock().expect("Could not lock mutex");
    let cooldown = cooldown.lock().expect("Could not lock mutex");
    if *last_message + *cooldown > timestamp {
        return false;
    }

    *last_message = timestamp;

    true
}

fn rust_response<'a>() -> &'a str {
    let i = random::<u8>() % 5;
    match i {
        1 => "RUST MENTIONED :crab: :crab: :crab:",
        2 => "<@216767618923757568>",
        3 => "Rust is simply the best programming language. Nothing else can compare. I am naming my kids Rust and Ferris.",
        4 => concat!("Launch the Polaris,\n", "the end doesn't scare us\n", "When will this cease?\n", "The warheads will all rust in peace!"),
        _ => "Rust? Oh, you mean the game?"
    }
}

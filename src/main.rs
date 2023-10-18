mod commands;
mod text_detection;
mod types;

use chrono::{DateTime, Duration, Utc};
use poise::builtins::register_application_commands_buttons;
use poise::serenity_prelude as serenity;
use poise::Event;
use std::sync::Mutex;

use types::{Context, Data, Error};

/// In minutes
const DEFAULT_TEXT_DETECT_COOLDOWN: i64 = 5;

#[tokio::main]
async fn main() {
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![register(), change_text_detect_cooldown()],
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(&ctx, &event, framework, data))
            },
            ..Default::default()
        })
        .token(std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN"))
        .intents(
            serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT,
        )
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    last_rust_response: Mutex::new(DateTime::<Utc>::from_timestamp(0, 0).unwrap()),
                    last_tkinter_response: Mutex::new(
                        DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
                    ),
                    last_arch_response: Mutex::new(DateTime::<Utc>::from_timestamp(0, 0).unwrap()),
                    text_detect_cooldown: Mutex::new(Duration::minutes(
                        DEFAULT_TEXT_DETECT_COOLDOWN,
                    )),
                })
            })
        });

    framework.run().await.unwrap();
}

#[poise::command(prefix_command)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    register_application_commands_buttons(ctx).await?;
    Ok(())
}

#[poise::command(prefix_command, slash_command)]
pub async fn change_text_detect_cooldown(
    ctx: Context<'_>,
    #[description = "The cooldown in minutes"] cooldown: i64,
) -> Result<(), Error> {
    {
        let mut text_detect_cooldown = ctx
            .data()
            .text_detect_cooldown
            .lock()
            .expect("Could not lock mutex");
        *text_detect_cooldown = Duration::minutes(cooldown);
    }
    ctx.say("Done!").await?;
    Ok(())
}

async fn event_handler(
    ctx: &serenity::Context,
    event: &Event<'_>,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        Event::Message { new_message } => {
            text_detection::text_detection(ctx, event, _framework, data, new_message).await
        }
        _ => Ok(()),
    }
}

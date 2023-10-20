mod commands;
mod config;
mod text_detection;
mod types;

use config::Config;
use types::{Context, Data, Error};

use poise::builtins::register_application_commands_buttons;
use poise::serenity_prelude as serenity;
use poise::Event;

#[tokio::main]
async fn main() {
    let config = Config::fetch();
    config.save();
    let data = Data::init(config);
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                register(),
                commands::change_text_detect_cooldown(),
                commands::create_class_category(),
                commands::reload_config(),
            ],
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .token(data.config.get_token())
        .intents(
            serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT,
        )
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(data)
            })
        });

    framework.run().await.unwrap();
}

#[poise::command(prefix_command)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    register_application_commands_buttons(ctx).await?;
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

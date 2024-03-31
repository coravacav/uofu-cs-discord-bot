mod commands;
pub mod config;
mod data;
mod event_handler;
mod handle_starboards;
mod lang;
mod starboard;
mod text_detection;

use color_eyre::eyre::{Error, Result};
use config::Config;
use data::AppState;
use event_handler::event_handler;
use poise::serenity_prelude as serenity;

/// Create the framework for the bot.
///
/// Split from the main function so that the main function can focus on cli arguments and starting
pub async fn create_framework(config: Config) -> Result<poise::FrameworkBuilder<AppState, Error>> {
    Ok(poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::get_commands(),
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            on_error: |error| {
                async fn on_error(error: poise::FrameworkError<'_, AppState, Error>) {
                    tracing::error!("{}", error);
                    dbg!(error);
                }

                Box::pin(on_error(error))
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_in_guild(
                    ctx,
                    &framework.options().commands,
                    serenity::GuildId::from(config.guild_id),
                )
                .await?;
                Ok(AppState::new(config))
            })
        }))
}

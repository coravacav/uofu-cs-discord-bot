mod commands;
pub mod config;
mod data;
mod event_handler;
mod handle_starboards;
mod lang;
mod memory_regex;
mod starboard;
mod text_detection;

use config::Config;
use data::Data;
use event_handler::event_handler;
use poise::serenity_prelude as serenity;

pub async fn create_framework(
    config: Config,
) -> anyhow::Result<poise::FrameworkBuilder<Data, anyhow::Error>> {
    Ok(poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::get_commands(),
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
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
                Ok(Data::new(config))
            })
        }))
}

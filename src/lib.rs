mod commands;
pub mod config;
mod data;
mod event_handler;
mod lang;
mod memory_regex;
mod reaction_management;
mod text_detection;

use anyhow::Context;
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
        .token(std::env::var("DISCORD_TOKEN").context("Expected a discord token")?)
        .intents(
            serenity::GatewayIntents::non_privileged()
                | serenity::GatewayIntents::MESSAGE_CONTENT
                | serenity::GatewayIntents::GUILD_MEMBERS
                | serenity::GatewayIntents::GUILD_MESSAGE_REACTIONS
                | serenity::GatewayIntents::GUILD_MESSAGES,
        )
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_in_guild(
                    ctx,
                    &framework.options().commands,
                    serenity::GuildId::from(1065373537591894086),
                )
                .await?;
                Ok(Data::new(config))
            })
        }))
}

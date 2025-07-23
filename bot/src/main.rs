use std::sync::Arc;

use bot_lib::{
    commands::*,
    config,
    data::{RawAppState, State},
    event_handler::event_handler,
};
use clap::Parser;
use color_eyre::eyre::{Result, WrapErr};
use dotenvy::dotenv;
use poise::serenity_prelude as serenity;
use tracing_subscriber::prelude::*;

/// The CLI arguments for the bot
///
/// In general, not used very often, but, can be nice for testing.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Don't start the discord bot, do all setup checks.
    #[arg(short, long, default_value = "false")]
    pub dry_run: bool,

    /// Path to the config file, if you want to use a different one.
    #[arg(short, long, default_value_t = String::from("config.toml"))]
    pub config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().wrap_err("Failed to load .env file. Add a file with the following contents: `DISCORD_TOKEN=\"your token\"` to a .env file in the root directory of the repo.")?;
    color_eyre::install()?;

    tracing_subscriber::registry()
        // .with(console_subscriber::spawn())
        .with(tracing_subscriber::fmt::layer().compact().with_filter(
            tracing_subscriber::filter::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::filter::EnvFilter::new(
                    "serenity::gateway::shard=off,serenity=warn,bot=info,bot-lib=info",
                )
            }),
        ))
        .init();

    let Args {
        dry_run,
        config: config_path,
    } = Args::parse();

    let token =
        std::env::var("DISCORD_TOKEN").wrap_err("Expected a discord token environment variable")?;
    let config =
        config::Config::create_from_file(&config_path).wrap_err("Failed to load config")?;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                // course_catalog_search(),
                add_bot_role(),
                add_class_role(),
                add_dog_role(),
                anon_notify(),
                aur_search(),
                bank_admin(),
                bank(),
                catalog(),
                clip_that(),
                coinflip(),
                course_request(),
                create_class_category(),
                debug_print_channel_names(),
                db_admin(),
                delete_class_category(),
                extract_all_class_channels(),
                extract_current_channel(),
                help(),
                list_classes(),
                mod_abuse(),
                my_classes(),
                parry(),
                remove_bot_role(),
                remove_class_role(),
                remove_dog_role(),
                reroll_reply(),
                reset_class_category(),
                reset_all_class_categories(),
                sathya(),
                search_catalog(),
                send_feedback(),
                timeout(),
                yeet_leaderboard(),
                yeet(),
            ],
            event_handler: |ctx, event, _framework, data| {
                Box::pin(event_handler(ctx, event, data.clone()))
            },
            on_error: |error| {
                async fn on_error(
                    error: poise::FrameworkError<'_, State, color_eyre::eyre::Error>,
                ) {
                    // Don't care.
                    if let poise::FrameworkError::CommandCheckFailed { .. }
                    | poise::FrameworkError::MissingUserPermissions { .. } = error
                    {
                        return;
                    }

                    tracing::error!("{}", error.to_string());
                }

                Box::pin(on_error(error))
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            tokio::spawn(async { update_interval().await });

            Box::pin(async move {
                // Register in guild is faster - but, makes testing and other things harder.
                // Restarting discord for new commands is plenty fine (or just waiting for the cache bust).
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                Ok(Arc::new(RawAppState::new(config, config_path).unwrap()))
            })
        });

    let client = serenity::ClientBuilder::new(
        token,
        serenity::GatewayIntents::non_privileged()
            | serenity::GatewayIntents::MESSAGE_CONTENT
            | serenity::GatewayIntents::GUILD_MEMBERS
            | serenity::GatewayIntents::GUILD_MESSAGE_REACTIONS
            | serenity::GatewayIntents::GUILD_MESSAGES,
    )
    .framework(framework.build())
    .await;

    notify_on_executable_update()?;

    if dry_run {
        println!("Bot setup worked, dry run enabled, exiting");
        return Ok(());
    }

    tracing::info!("Starting bot");

    client
        .wrap_err("Failed to start bot (serenity)")?
        .start()
        .await
        .wrap_err("Failed to start bot (startup / runtime)")
}

fn notify_on_executable_update() -> Result<()> {
    use notify::EventKind;
    use notify::RecursiveMode::NonRecursive;
    use notify::Watcher;
    use notify::event::CreateKind;

    let current_exe = std::env::current_exe()?;
    let directory = current_exe.parent().unwrap().to_owned();

    let mut watcher = notify::recommended_watcher(move |res| match res {
        Ok(notify::Event {
            kind: EventKind::Create(CreateKind::File),
            paths,
            ..
        }) => {
            if let Some(true) = paths.first().map(|p| p == &current_exe) {
                tracing::info!("executable updated!");
            }
        }
        Err(e) => tracing::error!("watch error: {:?}", e),
        _ => {}
    })?;

    watcher.watch(&directory, NonRecursive)?;

    // Don't want to drop it. Also, only done once, so it's fine.
    Box::leak(Box::new(watcher));

    Ok(())
}

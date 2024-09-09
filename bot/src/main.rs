use bot_lib::{commands::*, config, data::AppState, event_handler::event_handler};
use clap::Parser;
use color_eyre::eyre::{Result, WrapErr};
use dotenvy::dotenv;
use poise::serenity_prelude as serenity;
use tracing_subscriber::prelude::*;

/// The cli arguments for the bot
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Don't start the discord bot
    #[arg(short, long, default_value = "false")]
    pub dry_run: bool,

    /// Path to the config file
    #[arg(short, long, default_value_t = String::from("config.toml"))]
    pub config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().wrap_err("Failed to load .env file. Add a file with the following contents: `DISCORD_TOKEN=\"your token\"` to a .env file in the root directory of the repo.")?;
    color_eyre::install()?;

    tracing_subscriber::registry()
        .with(console_subscriber::spawn())
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_file(true)
                .with_line_number(true)
                .with_filter(
                    tracing_subscriber::filter::EnvFilter::try_from_default_env().unwrap_or_else(
                        |_| {
                            tracing_subscriber::filter::EnvFilter::new(
                                "serenity=warn,bot=info,bot-lib=info",
                            )
                        },
                    ),
                ),
        )
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
                aur_search(),
                add_bot_role(),
                help(),
                create_class_category(),
                course_catalog(),
                register(),
                remove_bot_role(),
                timeout(),
                yeet(),
                coinflip(),
                reset_class_category(),
                // course_catalog_search(),
                send_feedback(),
                reset_class_categories(),
                delete_class_category(),
                yeet_leaderboard(),
                add_dog_role(),
                bank(),
                bank_admin(),
                db_admin(),
                remove_dog_role(),
                update_class_category(),
                add_class_role(),
                sathya(),
                llm_prompt(),
                remove_class_role(),
            ],
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            on_error: |error| {
                async fn on_error(
                    error: poise::FrameworkError<'_, AppState, color_eyre::eyre::Error>,
                ) {
                    // Don't care.
                    if let poise::FrameworkError::CommandCheckFailed { .. } = error {
                        return;
                    }

                    tracing::error!("{:?}", error);
                }

                Box::pin(on_error(error))
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            tokio::spawn(async { update_interval().await });

            Box::pin(async move {
                poise::builtins::register_in_guild(
                    ctx,
                    &framework.options().commands,
                    serenity::GuildId::from(config.guild_id),
                )
                .await?;

                Ok(AppState::new(config, config_path).unwrap())
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
        .wrap_err("Failed to start bot (startup)")
}

fn notify_on_executable_update() -> Result<()> {
    use notify::event::CreateKind;
    use notify::EventKind;
    use notify::RecursiveMode::NonRecursive;
    use notify::Watcher;

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

    // Don't drop it!
    std::mem::forget(watcher);

    Ok(())
}

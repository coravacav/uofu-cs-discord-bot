use bot_lib::{
    commands::{
        aur_search::aur_search,
        class_commands::{
            add_class_role, create_class_category, delete_class_category, remove_class_role,
            reset_class_categories, reset_class_category, update_class_category,
        },
        course_catalog::course_catalog,
        help::help,
        llm_prompt::llm_prompt,
        lynch::{lynch, lynch_leaderboard, update_interval},
        register::register,
        sathya::sathya,
        set_bot_role::{add_bot_role, remove_bot_role},
        set_dog_role::{add_dog_role, remove_dog_role},
        timeout::timeout,
    },
    config,
    data::AppState,
    event_handler::event_handler,
};
use clap::Parser;
use color_eyre::eyre::{Result, WrapErr};
use dotenvy::dotenv;
use poise::serenity_prelude as serenity;
use tokio::io::{stdin, AsyncReadExt};
use tracing_subscriber::util::SubscriberInitExt;

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

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .finish()
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
                lynch(),
                reset_class_category(),
                reset_class_categories(),
                delete_class_category(),
                lynch_leaderboard(),
                add_dog_role(),
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

    if dry_run {
        println!("Bot setup worked, dry run enabled, exiting");
        return Ok(());
    }

    tokio::task::spawn(async {
        let mut stdin = stdin();
        let mut key = [0; 1];
        loop {
            stdin
                .read_exact(&mut key)
                .await
                .inspect_err(|e| {
                    tracing::error!("Failed to read from stdin: {:?}", e);
                })
                .ok();

            // This will be expanded later
            match key[0] {
                b'd' => {
                    println!("Debug");
                }
                b's' => {
                    println!("Status check");
                }
                _ => {}
            }
        }
    });

    tracing::info!("Starting bot");

    client
        .wrap_err("Failed to start bot (serenity)")?
        .start()
        .await
        .wrap_err("Failed to start bot (startup)")
}

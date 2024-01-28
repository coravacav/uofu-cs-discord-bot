use clap::Parser;
use color_eyre::eyre::{bail, Context, Result};
use dotenvy::dotenv;
use poise::serenity_prelude as serenity;
use uofu_cs_discord_bot::{config, create_framework};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Don't start the discord bot
    #[arg(short, long, default_value = "false")]
    pub dry_run: bool,

    /// Number of times to greet
    #[arg(short, long, default_value_t = String::from("config.toml"))]
    pub config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().context("Failed to load .env file")?;
    color_eyre::install()?;

    let args = Args::parse();
    let token = std::env::var("DISCORD_TOKEN").context("Expected a discord token")?;

    let config = config::Config::create_from_file(&args.config).expect("Failed to load config");

    let framework = create_framework(config).await;

    let Ok(framework) = framework else {
        bail!("Failed to create framework");
    };

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

    client
        .context("Failed to start bot (serenity)")?
        .start()
        .await
        .context("Failed to start bot (startup)")
}

use anyhow::Context;
use clap::Parser;
use dotenvy::dotenv;
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
async fn main() -> anyhow::Result<()> {
    dotenv().context("Failed to load .env file")?;

    let args = Args::parse();

    let config = config::Config::create_from_file(&args.config).expect("Failed to load config");

    let framework = create_framework(config).await;

    let Ok(framework) = framework else {
        return Err(anyhow::anyhow!("Failed to create framework"));
    };

    framework.run().await.context("Failed to start bot")
}

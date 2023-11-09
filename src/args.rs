use clap::Parser;

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

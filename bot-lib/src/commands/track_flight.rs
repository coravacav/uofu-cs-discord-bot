use poise::{CreateReply, serenity_prelude as serenity};
use regex::Regex;

const BASE_FLIGHTAWARE_URL: &str = "https://www.flightaware.com/";

///get information on a specified flight
#[poise::command(slash_command, rename = "track flight")]
pub async fn track_flight(
    ctx: PoiseContext<'_>,
    search: String,
) -> Result<()> {
    ctx.defer().await?;

    let search: String = search
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>();

    let re = Regex::new(r"[A-Za-z]{2,3}[0-9]{1,4}").unwrap();

    if search.len() < 2 || search.is_empty() ||  re.is_match(search) {
        ctx.reply("Please provide a valid  flight number").await?;
        return Ok(());
    }


}
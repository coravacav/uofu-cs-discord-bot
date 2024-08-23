use crate::data::PoiseContext;
use color_eyre::eyre::Result;
use itertools::Itertools;
use poise::{serenity_prelude as serenity, CreateReply};
use raur::Raur;
use std::cmp::Reverse;

const SEARCHCAP: usize = 20;
const BASE_AUR_URL: &str = "https://aur.archlinux.org/packages/";
#[poise::command(slash_command, prefix_command, rename = "aur")]
pub async fn aur_search(ctx: PoiseContext<'_>, search: String, amount: usize) -> Result<()> {
    ctx.defer().await?;

    let search: String = search
        .to_uppercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>();

    if search.is_empty() {
        ctx.reply("Please provide a valid search string").await?;
        return Ok(());
    }

    if amount > SEARCHCAP || amount < 1 {
        ctx.reply(format!(
            "Please provide an amount between 1 and {}",
            SEARCHCAP
        ))
        .await?;
        return Ok(());
    }

    let raur = raur::Handle::new();

    let pkgs = raur.search(search).await?;

    if pkgs.is_empty() {
        ctx.send(
            CreateReply::default()
                .embed(
                    serenity::CreateEmbed::new()
                        .title("No Packages found!")
                        .description("Please try with a different query!"),
                )
                .reply(true),
        )
        .await?;

        return Ok(());
    }

    let pkgs_iter = pkgs
        .iter()
        .sorted_by_key(|pkg| Reverse(pkg.num_votes))
        .take(amount);
    let mut pretty_results = "".to_string();

    for pkg in pkgs_iter {
        let version = pkg.version.as_str();
        let name = pkg.name.as_str();
        let url = format!("{BASE_AUR_URL}{name}");
        let votes = pkg.num_votes;

        let formatted_info = format!(
            "- [{}]({}) - Version {} Votes: {} \n",
            name, url, version, votes
        );
        pretty_results = pretty_results + &formatted_info;
    }

    ctx.send(
        CreateReply::default()
            .embed(
                serenity::CreateEmbed::new()
                    .title(format!(
                        "Found {} packages - Displaying top {}",
                        pkgs.len(),
                        amount
                    ))
                    .description(pretty_results),
            )
            .reply(true),
    )
    .await?;

    return Ok(());
}

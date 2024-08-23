use crate::data::PoiseContext;
use color_eyre::eyre::Result;
use itertools::Itertools;
use poise::{serenity_prelude as serenity, CreateReply};
use raur::Raur;
use std::cmp::Reverse;

const BASE_AUR_URL: &str = "https://aur.archlinux.org/packages/";
/// A simple command to search the aur, you cannot display more than 20 results at a time.
#[poise::command(slash_command, rename = "aur")]
pub async fn aur_search(
    ctx: PoiseContext<'_>,
    search: String,
    #[min = 1]
    #[max = 20]
    amount: usize,
) -> Result<()> {
    ctx.defer().await?;

    let search: String = search
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>();

    if search.is_empty() {
        ctx.reply("Please provide a valid search string").await?;
        return Ok(());
    }

    let raur = raur::Handle::new();

    let pkgs = raur.search(&search).await?;

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
        let version = &pkg.version;
        let name = &pkg.name;
        let votes = pkg.num_votes;

        let formatted_info =
            format!("- [{name}]({BASE_AUR_URL}{name}) `{version}` ({votes} votes) \n",);
        pretty_results = format!("{}{}", pretty_results, &formatted_info);
    }

    ctx.send(
        CreateReply::default()
            .embed(
                serenity::CreateEmbed::new()
                    .title(format!(
                        "Found {} packages from search query \"{search}\" - Displaying top {}",
                        pkgs.len(),
                        amount
                    ))
                    .description(pretty_results),
            )
            .reply(true),
    )
    .await?;

    Ok(())
}

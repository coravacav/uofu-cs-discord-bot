use crate::data::PoiseContext;
use color_eyre::eyre::Result;
use poise::{serenity_prelude as serenity, CreateReply};
use raur::Raur;

const SEARCHCAP: usize = 20;

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
    let pkgs_iter = pkgs.iter().take(amount);
    let mut pretty_results = "".to_string();

    for pkg in pkgs_iter {
        let version = pkg.version.as_str();
        let name = pkg.name.as_str();
        let url = pkg.url.as_ref().unwrap().as_str();

        let formatted_info = format!("({})[{}] - Version {} \n", name, url, version);
        pretty_results = pretty_results + &formatted_info;
    }

    ctx.send(
        CreateReply::default()
            .embed(
                serenity::CreateEmbed::new()
                    .title(format!(
                        "Found {} packages - Displaying top {}",
                        pkgs.len(),
                        SEARCHCAP
                    ))
                    .description(pretty_results),
            )
            .reply(true),
    )
    .await?;

    return Ok(());
}

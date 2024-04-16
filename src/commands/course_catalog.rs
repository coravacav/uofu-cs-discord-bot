use crate::data::PoiseContext;
use color_eyre::eyre::Result;
use poise::{serenity_prelude as serenity, CreateReply};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CatalogEntry {
    #[serde(rename = "pid")]
    page_id: String,
    title: String,
    description: String,
    code: String,
}

#[poise::command(slash_command, prefix_command, rename = "catalog")]
pub async fn course_catalog(ctx: PoiseContext<'_>, course_name: String) -> Result<()> {
    ctx.defer().await?;

    let formatted_course_name = course_name
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();

    // The catalog ID is just hard-coded for now.
    // I'm not sure if there's a better/dynamic way to find this.
    // See <https://developers.kuali.co/#header-hosting> for more info.
    let catalog_id = "6529bbfa1170af001cdefde1";
    let catalog_url = format!("https://utah.kuali.co/api/v1/catalog/search/{catalog_id}");

    let client = reqwest::Client::new();
    let response = client
        .get(catalog_url)
        .query(&[("q", formatted_course_name.as_str())])
        .send()
        .await?
        .error_for_status()?;

    let data: Vec<CatalogEntry> = response.json().await?;

    if let Some(entry) = data.first() {
        let title = format!("{} - {}", entry.code, entry.title);
        let class_url = format!("https://catalog.utah.edu/#/courses/{}", entry.page_id);

        let embed = serenity::CreateEmbed::new()
            .title(title)
            .description(&entry.description)
            .url(class_url);
        ctx.send(CreateReply::default().embed(embed).reply(true))
            .await?;
    } else {
        ctx.reply("Sorry, I could not find that course! :pensive:")
            .await?;
    }

    Ok(())
}

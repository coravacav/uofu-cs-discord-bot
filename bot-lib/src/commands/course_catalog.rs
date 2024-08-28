use crate::{courses::get_course, data::PoiseContext};
use color_eyre::eyre::Result;
use poise::{serenity_prelude as serenity, CreateReply};

#[poise::command(slash_command, prefix_command, rename = "catalog")]
pub async fn course_catalog(ctx: PoiseContext<'_>, course_id: String) -> Result<()> {
    ctx.defer().await?;

    let mut course_id = course_id
        .to_uppercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>();

    if course_id.is_empty() {
        ctx.reply("Please provide a valid course id").await?;
        return Ok(());
    }

    if course_id.chars().next().unwrap().is_numeric() {
        course_id = format!("CS{}", course_id);
    }

    let Some(course) = get_course(&course_id) else {
        ctx.reply(format!("Could not find a course with id {}", course_id))
            .await?;
        return Ok(());
    };

    ctx.send(
        CreateReply::default()
            .embed(
                serenity::CreateEmbed::new()
                    .title(format!(
                        "{} - {}{}",
                        course_id,
                        course.long_name.clone(),
                        if course.are_there_duplicates {
                            " Note: maybe a duplicate, the U has bad APIs"
                        } else {
                            ""
                        }
                    ))
                    .description(course.description.clone())
                    .url(format!(
                        "https://catalog.utah.edu/courses/{}",
                        course.course_group_id
                    )),
            )
            .reply(true),
    )
    .await?;

    Ok(())
}

// #[poise::command(slash_command, prefix_command, rename = "catalog_search")]
// pub async fn course_catalog_search(ctx: PoiseContext<'_>, search_string: String) -> Result<()> {
//     ctx.defer().await?;

//     let mut search_string = search_string
//         .to_uppercase()
//         .collect::<String>();

//     if search_string.is_empty() {
//         ctx.reply("Please provide a valid search string").await?;
//         return Ok(());
//     }
// }

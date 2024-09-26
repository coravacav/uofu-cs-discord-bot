use crate::{
    courses::{get_course, Course, COURSES},
    data::PoiseContext,
};
use color_eyre::eyre::Result;
use itertools::Itertools;
use poise::{serenity_prelude as serenity, CreateReply};

#[poise::command(slash_command)]
pub async fn catalog(ctx: PoiseContext<'_>, course_id: String) -> Result<()> {
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

    ctx.send(get_course_reply(&course)).await?;

    Ok(())
}

fn get_course_reply(course: &Course) -> CreateReply {
    CreateReply::default()
        .embed(
            serenity::CreateEmbed::new()
                .title(format!(
                    "{} - {}{}",
                    course.course_id,
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
        .reply(true)
}

/// Searches the U of U course catalog based on a search string
///
/// Searches the course code, title, and description
#[poise::command(slash_command)]
pub async fn search_catalog(ctx: PoiseContext<'_>, search_string: String) -> Result<()> {
    if search_string.is_empty() {
        ctx.reply("Please provide a valid search string").await?;
        return Ok(());
    }

    let upper_search_string = search_string.to_uppercase();

    let reply = {
        let courses = COURSES.read();

        let courses = courses
            .values()
            .filter(|course| {
                [
                    course.course_id.as_str(),
                    course.long_name.as_str(),
                    course.description.as_str(),
                ]
                .iter()
                .any(|string_to_search| {
                    string_to_search
                        .to_uppercase()
                        .contains(&upper_search_string)
                })
            })
            .take(20)
            .collect_vec();

        let course_count = courses.len();

        match course_count {
            0 => CreateReply::default()
                .content(format!("No courses found for \"{}\"", search_string))
                .reply(true)
                .ephemeral(true),
            1 => get_course_reply(courses.first().unwrap()),
            _ => CreateReply::default()
                .content(format!(
                    "Found (at least) {} courses for \"{}\"\n{}",
                    course_count,
                    search_string,
                    courses
                        .iter()
                        .map(|course| format!("`{}`", course.course_id))
                        .join(" ")
                ))
                .reply(true)
                .ephemeral(true),
        }
    };

    ctx.send(reply).await?;

    Ok(())
}

// #[poise::command(slash_command, rename = "catalog_search")]
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

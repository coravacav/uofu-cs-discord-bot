use crate::data::PoiseContext;
use color_eyre::eyre::Result;
use poise::{serenity_prelude as serenity, CreateReply};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize, Default)]
struct CourseList(Vec<Course>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
struct Course {
    #[serde(rename = "__catalogCourseId")]
    course_id: String,
    pid: String,
    title: String,
}

// For now we are only going to fetch the data once at the start of the bot.
static COURSES: OnceLock<CourseList> = OnceLock::new();
const U_OF_U_COURSE_API_ID: &str = "6529bbfa1170af001cdefde1";

#[poise::command(slash_command, prefix_command, rename = "catalog")]
pub async fn course_catalog(ctx: PoiseContext<'_>, course_id: String) -> Result<()> {
    ctx.defer().await?;

    let mut course_id = course_id
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>();

    if course_id.is_empty() {
        ctx.reply("Please provide a valid course id").await?;
        return Ok(());
    }

    if course_id.chars().next().unwrap().is_numeric() {
        course_id = format!("cs{}", course_id);
    }

    let courses = COURSES.get_or_init(|| {
        reqwest::blocking::get(format!(
            "https://utah.kuali.co/api/v1/catalog/courses/{U_OF_U_COURSE_API_ID}"
        ))
        .map(|body| body.json().unwrap_or_default())
        .unwrap_or_default()
    });

    if courses.0.is_empty() {
        ctx.reply("The course list couldn't be loaded! Let the mods know.")
            .await?;
        return Ok(());
    }

    let Some(course) = courses
        .0
        .iter()
        .find(|course| course.course_id.to_lowercase() == course_id)
    else {
        ctx.reply(format!("Could not find a course with id {}", course_id))
            .await?;
        return Ok(());
    };

    let course_url = format!("https://catalog.utah.edu/#/courses/{}", course.pid);

    ctx.send(
        CreateReply::default()
            .embed(
                serenity::CreateEmbed::new()
                    .title(format!("{} - {}", course.course_id, course.title))
                    .description(
                        get_description(&course.pid)
                            .await
                            .unwrap_or(String::from("Could not get description")),
                    )
                    .url(course_url),
            )
            .reply(true),
    )
    .await?;

    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
struct CourseInformation {
    description: String,
}

async fn get_description(course_pid: &str) -> Result<String> {
    let data_url = format!(
        "https://utah.kuali.co/api/v1/catalog/course/{}/{}",
        U_OF_U_COURSE_API_ID, course_pid
    );

    dbg!(&data_url);

    let course_data: CourseInformation = reqwest::get(data_url).await?.json().await?;

    let description = course_data.description;

    Ok(description.to_string())
}

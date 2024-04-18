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

#[poise::command(slash_command, prefix_command, rename = "catalog")]
pub async fn course_catalog(ctx: PoiseContext<'_>, course_id: String) -> Result<()> {
    ctx.defer().await?;

    let courses = COURSES.get_or_init(|| {
        reqwest::blocking::get(
            "https://utah.kuali.co/api/v1/catalog/courses/6529bbfa1170af001cdefde1",
        )
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
        .find(|course| course.course_id == course_id)
    else {
        ctx.reply("Could not find a course with that id!").await?;
        return Ok(());
    };

    let course_url = format!("https://catalog.utah.edu/#/courses/{}", course.pid);

    ctx.send(
        CreateReply::default()
            .embed(
                serenity::CreateEmbed::new()
                    .title(format!("{} - {}", course.course_id, course.title))
                    .url(course_url),
            )
            .reply(true),
    )
    .await?;

    Ok(())
}

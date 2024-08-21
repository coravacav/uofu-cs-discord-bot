use crate::data::PoiseContext;
use color_eyre::eyre::Result;
use dashmap::DashMap;
use poise::{serenity_prelude as serenity, CreateReply};
use serde::Deserialize;
use std::sync::LazyLock;

static COURSES: LazyLock<DashMap<String, Course>> = LazyLock::new(|| {
    let static_json_file = include_str!("../../../classes.json");

    let file: File = serde_json::from_str(static_json_file).unwrap();

    let courses = DashMap::new();

    for mut course in file.data {
        let current = courses.get(&course.course_id);
        let are_there_duplicates = current.is_some();
        drop(current);
        course.are_there_duplicates = are_there_duplicates;
        courses.insert(course.course_id.clone(), course);
    }

    tracing::info!("Loaded {} courses", courses.len());

    courses
});

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
struct File {
    data: Vec<Course>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
struct Course {
    #[serde(rename = "code")]
    course_id: String,
    long_name: String,
    description: String,
    course_group_id: String,
    #[serde(skip)]
    are_there_duplicates: bool,
}

// https://app.coursedog.com/api/v1/cm/utah_peoplesoft/courses/search/%24filters?skip=0&limit=20000&columns=customFields.rawCourseId%2CdisplayName%2Cdepartment%2Cdescription%2Cname%2CcourseNumber%2CsubjectCode%2Ccode%2CcourseGroupId%2Ccareer%2Ccollege%2ClongName%2Cstatus14

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

    let Some(course) = COURSES.get(&course_id).map(|c| c.clone()) else {
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
                            "Note: maybe a duplicate, the U has bad APIs"
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

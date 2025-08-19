use std::sync::Arc;

use crate::{
    courses::{COURSES, Course, CourseIdent, CourseStatus, get_course},
    data::PoiseContext,
    utils::SendReplyEphemeral,
};
use color_eyre::eyre::Result;
use itertools::Itertools;
use poise::{CreateReply, serenity_prelude as serenity};

/// Enter the course id of a course you want to see the catalog for.
///
/// If prefix is ommitted, it will assume it's CS.
#[poise::command(slash_command)]
pub async fn catalog(
    ctx: PoiseContext<'_>,
    #[description = "The course identifier (auto adds \"CS\" if unspecified)"] course_id: String,
) -> Result<()> {
    let Ok(course) = CourseIdent::try_from(course_id.as_str()) else {
        ctx.reply_ephemeral(format!("Please provide a valid course, got `{course_id}`"))
            .await?;
        return Ok(());
    };

    let Some(course) = get_course(&course) else {
        ctx.reply_ephemeral(format!("Could not find a course with id `{course_id}`"))
            .await?;
        return Ok(());
    };

    // Use the shared cached reply helper so we don't rebuild embeds repeatedly.
    ctx.send(get_or_cache_course_reply(&course)).await?;

    Ok(())
}

fn make_course_reply(course: &Course) -> CreateReply {
    CreateReply::default()
        .embed(
            serenity::CreateEmbed::new()
                .title(format!(
                    "{}{} - {}{}",
                    course.course_id,
                    match course.status {
                        CourseStatus::Active => "",
                        CourseStatus::Inactive => " (Inactive)",
                        _ => " (Unknown Status)",
                    },
                    course.long_name.clone(),
                    if course.are_there_duplicates {
                        " Note: maybe a duplicate, the U has bad APIs"
                    } else {
                        ""
                    }
                ))
                .description(course.description.clone())
                .url(if let Some(url) = &course.url_override {
                    url.clone()
                } else {
                    format!(
                        "https://catalog.utah.edu/courses/{}",
                        course.course_group_id.clone().unwrap_or_default()
                    )
                }),
        )
        .reply(true)
}

/// Return a cached reply for the course if present, otherwise build & cache it.
fn get_or_cache_course_reply(course: &Course) -> CreateReply {
    if let Some(message) = &course.cached_message {
        return message.clone();
    }

    let message = make_course_reply(course);
    let course_id = Arc::clone(&course.course_id);
    let message_copy = message.clone();
    // Spawn to avoid holding the write lock while constructing the embed above.
    tokio::spawn(async move {
        let mut handle = COURSES.write();
        if let Some(course) = handle.get_mut(&*course_id) {
            // Only set if still empty to avoid clobbering potential newer value.
            if course.cached_message.is_none() {
                course.cached_message = Some(message_copy);
            }
        }
    });
    message
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
        let handle = COURSES.read();

        let courses = handle
            .values()
            .filter(|course| {
                [
                    &*course.course_id as &str,
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
                .content(format!("No courses found for \"{search_string}\""))
                .reply(true)
                .ephemeral(true),
            1 => get_or_cache_course_reply(courses.first().unwrap()),
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

/// Posts a link for course requests
#[poise::command(slash_command)]
pub async fn course_request(ctx: PoiseContext<'_>) -> Result<()> {
    ctx.send(
        CreateReply::default()
            .content("<https://www.cs.utah.edu/undergraduate/current-students/permission-codes/>")
            .reply(false),
    )
    .await?;

    Ok(())
}

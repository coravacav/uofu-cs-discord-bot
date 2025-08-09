use color_eyre::eyre::{Context, Result, eyre};
use parking_lot::RwLock;
use poise::CreateReply;
use serde::Deserialize;
use std::{
    collections::HashMap,
    io::Write,
    sync::{Arc, LazyLock},
};

pub(crate) static COURSES: LazyLock<RwLock<HashMap<String, Course>>> =
    LazyLock::new(Default::default);

pub fn get_course(course_id: &str) -> Option<Course> {
    COURSES.read().get(course_id).cloned()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
struct File {
    data: Vec<Course>,
}

#[derive(Deserialize, Clone, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum CourseStatus {
    Active,
    Inactive,
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Course {
    /// The course code, eg. CS2420
    #[serde(rename = "code")]
    pub course_id: Arc<String>,
    /// The long name of the course, eg. Introduction to Computer Science
    pub long_name: String,
    /// The description of the course
    pub description: String,
    /// Some arbitrary number the U gave
    pub course_group_id: Option<String>,
    /// URL override :)
    pub url_override: Option<String>,
    /// Whether or not there are duplicates. The U has a bad API.
    #[serde(skip)]
    pub are_there_duplicates: bool,
    /// The status of the course, eg. Active, Inactive
    pub status: CourseStatus,
    #[serde(skip)]
    /// A cached message for the course
    pub cached_message: Option<CreateReply>,
}

pub fn update_course_list() {
    tokio::spawn(async move {
        let mut pause = tokio::time::interval(std::time::Duration::from_secs(604_800));

        loop {
            pause.tick().await;
            timing_and_error_wrapper().await;
        }
    });
}

async fn timing_and_error_wrapper() {
    let start = std::time::Instant::now();
    let result = fetch_and_update().await;
    let elapsed = start.elapsed();
    if let Err(e) = result {
        tracing::error!("Error updating course list: {}", e);
    } else {
        tracing::info!(
            "Successfully updated course list in {}.{:03} seconds",
            elapsed.as_secs(),
            elapsed.subsec_millis()
        );
    }
}

async fn fetch_and_update() -> Result<()> {
    let client = reqwest::Client::new();

    // Base endpoint (kept "$filters" as in your original request;
    // change if your API expects a different value here).
    let base = "https://app.coursedog.com/api/v1/cm/utah_peoplesoft\
/courses/search/$filters";

    // columns list (kept from your original request)
    let columns = [
        // "attributes",
        "code",
        // "college",
        // "courseNumber", // Derive from code
        "courseTypicallyOffered",
        // "credits",
        // "crossListedCourses", // Course equivalencies should be all we want
        // "crseOfferNbr",
        // "customFields.catalogAttributes",
        // "customFields.fJUUs",
        // "customFields.OGXiP",
        // "customFields.rawCourseId",
        // "customFields.Vo847",
        // "customFields.z5i2t",
        "description",
        // "institution",
        // "institutionId",
        // "learningOutcomes", // TODO ?
        "longName",
        // "name", // Always use longName
        // "rawCourseId", // this is just courseGroupId without the last digit (first 6 maybe)?
        "status",
        // "subjectCode", // Derive from code
    ]
    .join(",");

    let params = [
        ("formatDependents", "false"),
        ("includeRelatedData", "true"),
        ("includeCrosslisted", "true"),
        ("limit", "100000"), // ! Lower for testing so we don't get rate limited :)
        ("includeCourseEquivalencies", "true"),
        // ("includeMappedDocumentItems", "true"),
        // ("includePending", "false"),
        ("returnResultsWithTotalCount", "false"),
        ("doNotDisplayAllMappedRevisionsAsDependencies", "true"),
        ("columns", &columns),
    ];

    let url = reqwest::Url::parse_with_params(base, &params)?;

    let resp = client
        .get(url)
    // Replicate browser headers from observed curl to reduce chance of server rejecting request.
    .header("Accept", "application/json, text/plain, */*")
    .header("Accept-Language", "en-US,en;q=0.9")
    .header("Origin", "https://catalog.utah.edu")
    .header("Priority", "u=1, i")
    .header("Referer", "https://catalog.utah.edu/")
    .header("Sec-CH-UA", "\"Not)A;Brand\";v=\"8\", \"Chromium\";v=\"138\", \"Google Chrome\";v=\"138\"")
    .header("Sec-CH-UA-Mobile", "?0")
    .header("Sec-CH-UA-Platform", "\"macOS\"")
    .header("Sec-Fetch-Dest", "empty")
    .header("Sec-Fetch-Mode", "cors")
    .header("Sec-Fetch-Site", "cross-site")
    .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36")
    .header("X-Requested-With", "catalog")
        .send()
        .await?;

    if !resp.status().is_success() {
        eprintln!("Request failed: {}", resp.status());
        eprintln!("{}", resp.text().await?);
        return Err(eyre!("Failed to fetch courses"));
    }

    let json_text = resp.text().await.wrap_err("Failed to get text")?;
    let json: File = serde_json::from_str(&json_text)?;

    let mut courses: HashMap<String, Course> = HashMap::new();

    for mut course in json.data {
        let current = courses.remove(&*course.course_id);

        let saved_course = if let Some(current) = current {
            if course.status == CourseStatus::Active {
                if current.status == CourseStatus::Active {
                    course.are_there_duplicates = true;
                }
                course
            } else {
                current
            }
        } else {
            course
        };

        courses.insert((*saved_course.course_id).clone(), saved_course);
    }

    let mut saved_courses = COURSES.write();
    *saved_courses = courses;

    let file = std::fs::File::create("debug.json")?;
    let mut writer = std::io::BufWriter::new(file);
    writer.write_all(json_text.as_bytes())?;

    Ok(())
}

// https://app.coursedog.com/api/v1/cm/utah_peoplesoft/courses/search/%24filters?skip=0&limit=20000&columns=customFields.rawCourseId%2CdisplayName%2Cdepartment%2Cdescription%2Cname%2CcourseNumber%2CsubjectCode%2Ccode%2CcourseGroupId%2Ccareer%2Ccollege%2ClongName%2Cstatus14

// https://catalog.utah.edu/courses

// https://app.coursedog.com/api/v1/ca/utah_peoplesoft/catalogs/KVZ6USppfIBMqzBMK6UB/courses/csv/filters?orderBy=catalogDisplayName%2CtranscriptDescription%2ClongName%2Cname&ignoreEffectiveDating=false&columns=customFields.rawCourseId%2CdisplayName%2Cdepartment%2Cdescription%2Cname%2CcourseNumber%2CsubjectCode%2Ccode%2CcourseGroupId%2Ccareer%2Ccollege%2ClongName%2Cstatus14

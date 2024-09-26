use parking_lot::RwLock;
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

pub(crate) static COURSES: LazyLock<RwLock<HashMap<String, Course>>> = LazyLock::new(|| {
    let instant = std::time::Instant::now();
    let file = std::fs::File::open("classes.json").unwrap();
    let file_reader = std::io::BufReader::new(file);
    let file: File = serde_json::from_reader(file_reader).unwrap();

    let mut courses: HashMap<String, Course> = HashMap::new();

    for mut course in file.data {
        let current = courses.get(&*course.course_id);
        let are_there_duplicates = current.is_some();
        course.are_there_duplicates = are_there_duplicates;
        courses.insert((*course.course_id).clone(), course);
    }

    let elapsed = instant.elapsed();

    tracing::info!(
        "Loaded {} courses in {}.{} seconds",
        courses.len(),
        elapsed.as_secs(),
        elapsed.subsec_millis()
    );

    courses.into()
});

pub fn get_course(course_id: &str) -> Option<Course> {
    COURSES.read().get(course_id).cloned()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
struct File {
    data: Vec<Course>,
}

#[derive(Debug, Deserialize, Clone)]
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
    pub course_group_id: String,
    /// Whether or not there are duplicates. The U has a bad API.
    #[serde(skip)]
    pub are_there_duplicates: bool,
}

// https://app.coursedog.com/api/v1/cm/utah_peoplesoft/courses/search/%24filters?skip=0&limit=20000&columns=customFields.rawCourseId%2CdisplayName%2Cdepartment%2Cdescription%2Cname%2CcourseNumber%2CsubjectCode%2Ccode%2CcourseGroupId%2Ccareer%2Ccollege%2ClongName%2Cstatus14

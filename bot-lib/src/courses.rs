use dashmap::DashMap;
use serde::Deserialize;
use std::sync::LazyLock;

static COURSES: LazyLock<DashMap<String, Course>> = LazyLock::new(|| {
    let static_json_file = include_str!("../../classes.json");

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

pub fn  get_course(course_id: &str) -> Option<Course> {
    COURSES.get(course_id).map(|c| c.clone())
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
    #[serde(rename = "code")]
    pub course_id: String,
    pub long_name: String,
    pub description: String,
    pub course_group_id: String,
    #[serde(skip)]
    pub are_there_duplicates: bool,
}

// https://app.coursedog.com/api/v1/cm/utah_peoplesoft/courses/search/%24filters?skip=0&limit=20000&columns=customFields.rawCourseId%2CdisplayName%2Cdepartment%2Cdescription%2Cname%2CcourseNumber%2CsubjectCode%2Ccode%2CcourseGroupId%2Ccareer%2Ccollege%2ClongName%2Cstatus14

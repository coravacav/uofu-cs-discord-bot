use chrono::{DateTime, Utc};

pub trait GetRelativeTimestamp {
    fn discord_relative_timestamp(&self) -> String;
}

impl GetRelativeTimestamp for DateTime<Utc> {
    fn discord_relative_timestamp(&self) -> String {
        format!("<t:{}:R>", self.timestamp())
    }
}

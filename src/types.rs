use std::sync::Mutex;
use chrono::{Duration, Utc, DateTime};
pub struct Data {
    pub last_rust_response: Mutex<DateTime<Utc>>,
    pub text_detect_cooldown: Mutex<Duration>,
}

// User data, which is stored and accessible in all command invocations
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
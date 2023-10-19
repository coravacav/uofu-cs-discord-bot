use chrono::{DateTime, Duration, Utc};
use std::sync::Mutex;
use crate::config::Config;

pub struct Data {
    pub last_rust_response: Mutex<DateTime<Utc>>,
    pub last_tkinter_response: Mutex<DateTime<Utc>>,
    pub last_arch_response: Mutex<DateTime<Utc>>,
    pub last_goop_response: Mutex<DateTime<Utc>>,
    pub last_1984_response: Mutex<DateTime<Utc>>,
    pub config: Config
}

// User data, which is stored and accessible in all command invocations
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

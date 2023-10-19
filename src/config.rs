use std::sync::{Mutex, MutexGuard};

use chrono::Duration;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// In minutes
const DEFAULT_TEXT_DETECT_COOLDOWN: i64 = 5;

pub struct Config {
    text_detect_cooldown: Mutex<Duration>,
    discord_token: String,
    responses: Mutex<Vec<MessageResponse>>
}

impl Config {
    /// Fetches the config from the config.toml file in the root directory.
    /// If the delay is missing, it will default to 5 minutes.
    /// If the discord token is missing, it will attempt to use the DISCORD_TOKEN environment variable.
    pub fn fetch() -> Config {
        let config_builder = match std::fs::read_to_string("./config.toml") {
            Ok(contents) => toml::from_str(&contents).expect("Error parsing config.toml"),
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => ConfigBuilder::empty(),
                _ => panic!("Error reading config.toml: {}", e),
            },
        };
        let text_detect_cooldown = match config_builder.text_detect_cooldown {
            Some(cooldown) => Duration::minutes(cooldown),
            None => Duration::minutes(DEFAULT_TEXT_DETECT_COOLDOWN),
        };
        let discord_token = match config_builder.discord_token {
            Some(token) => token,
            None => std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN"),
        };

        let config = Config {
            text_detect_cooldown: Mutex::new(text_detect_cooldown),
            discord_token,
            responses: Mutex::new(config_builder.responses)
        };

        config
    }

    /// Updates config.toml with the new cooldown, and updates the cooldown as well
    pub fn update_cooldown(&self, cooldown: Duration) {
        let mut text_detect_cooldown = self.lock_cooldown();
        *text_detect_cooldown = cooldown;

        self.save();
    }

    /// Adds a response to the config.toml file and the config.
    pub fn add_response(&self, response: MessageResponse) {
        let mut responses = self.responses.lock().unwrap();
        responses.push(response);

        self.save();
    }

    /// Removes a response from the config.toml file and the config.
    pub fn remove_response(&self, name: String) {
        let mut responses = self.responses.lock().unwrap();
        *responses = responses.iter().filter(|response| response.get_name() != name).cloned().collect();

        self.save();
    }

    /// Locks and returns the text detect cooldown.
    /// The mutex guard returned is guaranteed to be unlocked, so can be used immediately.
    pub fn lock_cooldown(&self) -> MutexGuard<Duration> {
        let text_detect_cooldown = self
            .text_detect_cooldown
            .lock()
            .expect("Could not lock mutex");

        text_detect_cooldown
    }

    pub fn lock_responses(&self) -> MutexGuard<Vec<MessageResponse>> {
        let responses = self.responses.lock().unwrap();

        responses
    }

    pub fn get_response(&self, name: String) -> MessageResponse {
        self.responses.lock().unwrap().iter()
            .find(|response| response.get_name() == name)
            .unwrap().clone()
    }

    pub fn get_token(&self) -> String {
        self.discord_token.clone()
    }

    pub fn save(&self) {
        let config_builder = ConfigBuilder {
            text_detect_cooldown: Some(self.lock_cooldown().num_minutes()),
            discord_token: Some(self.discord_token.clone()),
            responses: self.responses.lock().unwrap().clone()
        };
        let toml = toml::to_string(&config_builder).unwrap();

        std::fs::write("./config.toml", toml).expect("Could not write to config.toml");
    }
}

#[derive(Deserialize, Serialize)]
struct ConfigBuilder {
    text_detect_cooldown: Option<i64>,
    discord_token: Option<String>,
    responses: Vec<MessageResponse>
}

impl ConfigBuilder {
    fn empty() -> ConfigBuilder {
        ConfigBuilder {
            text_detect_cooldown: None,
            discord_token: None,
            responses: Vec::new()
        }
    }
}

#[derive(Deserialize,Serialize,Clone)]
pub enum MessageResponse {
    Text{name: String, pattern: String, content: String},
    RandomText{name: String, pattern: String, content: Vec<String>},
    Image{name: String, pattern: String, path: String},
    TextAndImage{name: String, pattern: String, content: String, path: String}
}

impl MessageResponse {
    pub fn get_name(&self) -> String {
        match self {
            MessageResponse::Text{name, ..} => name.clone(),
            MessageResponse::RandomText{name, ..} => name.clone(),
            MessageResponse::TextAndImage{name, ..} => name.clone(),
            MessageResponse::Image{name, ..} => name.clone()
        }
    }

    pub fn get_pattern(&self) -> Regex {
        match self {
            MessageResponse::Text{pattern, ..} => Regex::new(pattern).unwrap(),
            MessageResponse::RandomText{pattern, ..} => Regex::new(pattern).unwrap(),
            MessageResponse::TextAndImage{pattern, ..} => Regex::new(pattern).unwrap(),
            MessageResponse::Image{pattern, ..} => Regex::new(pattern).unwrap()
        }
    }
}
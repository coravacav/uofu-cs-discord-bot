use std::sync::{Mutex, MutexGuard};

use chrono::Duration;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// In minutes
const DEFAULT_TEXT_DETECT_COOLDOWN: i64 = 5;

pub struct Config {
    text_detect_cooldown: Mutex<Duration>,
    discord_token: String,
    responses: Mutex<Vec<MessageResponse>>,
}

impl Config {
    /// Fetches the config from the config.toml file in the root directory.
    /// If the delay is missing, it will default to 5 minutes.
    /// If the discord token is missing, it will attempt to use the DISCORD_TOKEN environment variable.
    pub fn fetch() -> Config {
        let ConfigBuilder {
            text_detect_cooldown,
            discord_token,
            responses,
        } = match std::fs::read_to_string("./config.toml") {
            Ok(contents) => toml::from_str(&contents).expect("Error parsing config.toml"),
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => ConfigBuilder::empty(),
                _ => panic!("Error reading config.toml: {}", e),
            },
        };

        let text_detect_cooldown = Mutex::new(text_detect_cooldown.map_or(
            Duration::minutes(DEFAULT_TEXT_DETECT_COOLDOWN),
            Duration::minutes,
        ));

        let discord_token = discord_token.map_or(
            std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN"),
            |token| token,
        );

        Config {
            text_detect_cooldown,
            discord_token,
            responses: Mutex::new(responses),
        }
    }

    /// Reloads the config.toml file and updates the configuration.
    /// Note that this does not update the discord token, only the cooldown and responses
    pub fn reload(&self) {
        let new_config = Config::fetch();
        *self.lock_cooldown() = *new_config.lock_cooldown();
        *self.lock_responses() = new_config.lock_responses().clone();
    }

    /// Updates config.toml with the new cooldown, and updates the cooldown as well
    pub fn update_cooldown(&self, cooldown: Duration) {
        {
            let mut text_detect_cooldown = self.lock_cooldown();
            *text_detect_cooldown = cooldown;
        }

        self.save();
    }

    /// Adds a response to the config.toml file and the config.
    pub fn add_response(&self, response: MessageResponse) {
        let mut responses = self
            .responses
            .lock()
            .expect("Could not lock mutex for add_response");
        responses.push(response);

        self.save();
    }

    /// Removes a response from the config.toml file and the config.
    pub fn remove_response(&self, name: String) {
        let mut responses = self
            .responses
            .lock()
            .expect("Could not lock mutex for remove_response");

        *responses = responses
            .iter()
            .filter(|response| response.get_name() != name)
            .cloned()
            .collect();

        self.save();
    }

    /// Locks and returns the text detect cooldown.
    /// The mutex guard returned is guaranteed to be unlocked, so can be used immediately.
    pub fn lock_cooldown(&self) -> MutexGuard<Duration> {
        let text_detect_cooldown = self
            .text_detect_cooldown
            .lock()
            .expect("Could not lock mutex in lock_cooldown");

        text_detect_cooldown
    }

    pub fn lock_responses(&self) -> MutexGuard<Vec<MessageResponse>> {
        let responses = self
            .responses
            .lock()
            .expect("Could not lock mutex in lock_responses");

        responses
    }

    pub fn get_response(&self, name: String) -> MessageResponse {
        self.responses
            .lock()
            .expect("Could not lock mutex in get_response")
            .iter()
            .find(|response| response.get_name() == name)
            .expect("Could not find response with name") // I can't be arsed to make this be correct rn.
            .clone()
    }

    pub fn get_token(&self) -> String {
        self.discord_token.clone()
    }

    pub fn save(&self) {
        let config_builder = ConfigBuilder {
            text_detect_cooldown: Some(self.lock_cooldown().num_minutes()),
            discord_token: Some(self.discord_token.clone()),
            responses: self
                .responses
                .lock()
                .expect("Could not lock mutex in save")
                .clone(),
        };
        let toml = toml::to_string(&config_builder).expect("Could not serialize config");

        std::fs::write("./config.toml", toml).expect("Could not write to config.toml");
    }
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
struct ConfigBuilder {
    text_detect_cooldown: Option<i64>,
    discord_token: Option<String>,
    responses: Vec<MessageResponse>,
}

impl ConfigBuilder {
    fn empty() -> ConfigBuilder {
        ConfigBuilder {
            text_detect_cooldown: None,
            discord_token: None,
            responses: Vec::new(),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum MessageResponseKind {
    Text { content: String },
    RandomText { content: Vec<String> },
    Image { path: String },
    TextAndImage { content: String, path: String },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct MessageResponse {
    pub name: String,
    pub pattern: String,
    #[serde(flatten)]
    // This makes it so it pretends the attributes of the enum are attributes of the struct
    pub kind: MessageResponseKind,
}

impl MessageResponse {
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_pattern(&self) -> Regex {
        Regex::new(&self.pattern).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn should_deserialize_properly() {
        let test_input = r#"[[responses]]
name = "1984"
pattern = "1984"
content = "literally 1984""#;

        let config: ConfigBuilder = toml::from_str(test_input).unwrap();

        assert_eq!(
            config.responses.first(),
            Some(&MessageResponse {
                name: "1984".to_string(),
                pattern: "1984".to_string(),
                kind: MessageResponseKind::Text {
                    content: "literally 1984".to_string(),
                },
            })
        );
    }
}

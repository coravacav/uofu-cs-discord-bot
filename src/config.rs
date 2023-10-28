use std::sync::{RwLock, RwLockReadGuard};

use chrono::Duration;
use serde::{Deserialize, Serialize};

use crate::lang::Ruleset;

/// In minutes
const DEFAULT_TEXT_DETECT_COOLDOWN: i64 = 5;

pub struct Config {
    text_detect_cooldown: RwLock<Duration>,
    discord_token: String,
    responses: RwLock<Vec<MessageResponse>>,
}

impl Config {
    pub fn get_cooldown(&self) -> RwLockReadGuard<Duration> {
        self.text_detect_cooldown
            .read()
            .expect("Could not read cooldown")
    }

    pub fn get_responses(&self) -> RwLockReadGuard<Vec<MessageResponse>> {
        self.responses.read().expect("Could not read responses")
    }

    fn fetch_config() -> ConfigBuilder {
        let config = std::fs::read_to_string("./config.toml").expect("Could not read config.toml");

        toml::from_str(&config).expect("Could not deserialize config.toml")
    }

    /// Fetches the config from the config.toml file in the root directory.
    /// If the delay is missing, it will default to 5 minutes.
    /// If the discord token is missing, it will attempt to use the DISCORD_TOKEN environment variable.
    pub fn fetch() -> Config {
        let ConfigBuilder {
            text_detect_cooldown,
            discord_token,
            responses,
        } = Config::fetch_config();

        let text_detect_cooldown = RwLock::new(Duration::minutes(text_detect_cooldown));
        let responses = RwLock::new(responses);

        Config {
            text_detect_cooldown,
            discord_token,
            responses,
        }
    }

    /// Reloads the config.toml file and updates the configuration.
    pub fn reload(&self) {
        let new_config = Config::fetch_config();
        *self
            .text_detect_cooldown
            .write()
            .expect("Could not write cooldown") =
            Duration::minutes(new_config.text_detect_cooldown);
        *self.responses.write().expect("Could not write responses") = new_config.responses;
    }

    /// Updates config.toml with the new cooldown, and updates the cooldown as well
    pub fn update_cooldown(&self, cooldown: Duration) {
        *self
            .text_detect_cooldown
            .write()
            .expect("Could not set cooldown") = cooldown;

        self.save();
    }

    /// Adds a response to the config.toml file and the config.
    pub fn add_response(&mut self, response: MessageResponse) {
        self.responses
            .write()
            .expect("Could not write responses")
            .push(response);
        self.save();
    }

    /// Removes a response from the config.toml file and the config.
    pub fn remove_response(&mut self, name: String) {
        self.responses
            .write()
            .expect("Could not write responses")
            .retain(|response| response.name != name);
        self.save();
    }

    pub fn get_response(&self, name: String) -> MessageResponse {
        self.responses
            .read()
            .expect("Could not read responses")
            .iter()
            .find(|response| response.name == name)
            .expect("Could not find response with name")
            .clone()
    }

    pub fn get_token(&self) -> String {
        self.discord_token.clone()
    }

    pub fn save(&self) {
        let config_builder = ConfigBuilder {
            text_detect_cooldown: self
                .text_detect_cooldown
                .read()
                .expect("could not read cooldown")
                .num_minutes(),
            discord_token: self.discord_token.clone(),
            responses: self
                .responses
                .read()
                .expect("could not read responses")
                .clone(),
        };

        let toml = toml::to_string(&config_builder).expect("Could not serialize config");

        std::fs::write("./config.toml", toml).expect("Could not write to config.toml");
    }
}

fn get_default_text_detect_cooldown() -> i64 {
    DEFAULT_TEXT_DETECT_COOLDOWN
}

fn get_default_discord_token() -> String {
    std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN")
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
struct ConfigBuilder {
    #[serde(default = "get_default_text_detect_cooldown")]
    text_detect_cooldown: i64,
    #[serde(default = "get_default_discord_token")]
    discord_token: String,
    responses: Vec<MessageResponse>,
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
    pub ruleset: Ruleset,
    #[serde(flatten)]
    // This makes it so it pretends the attributes of the enum are attributes of the struct
    pub kind: MessageResponseKind,
}

#[cfg(test)]
mod test {
    use crate::{
        lang::{Kind, Line},
        memory_regex::MemoryRegex,
    };

    use super::*;

    #[test]
    fn should_deserialize_properly() {
        let test_input = r#"
discord_token = "test_token_not_real"
[[responses]]
name = "1984"
ruleset = '''
r 1234
!r 4312
'''
content = "literally 1984""#;

        let config: ConfigBuilder = toml::from_str(test_input).unwrap();

        assert_eq!(
            config.responses.first(),
            Some(&MessageResponse {
                name: "1984".to_string(),
                ruleset: Ruleset::new(vec![
                    Line {
                        kind: Kind::Regex(MemoryRegex::new("1234".to_string()).unwrap()),
                        negated: false,
                    },
                    Line {
                        kind: Kind::Regex(MemoryRegex::new("4312".to_string()).unwrap()),
                        negated: true,
                    }
                ]),
                kind: MessageResponseKind::Text {
                    content: "literally 1984".to_string(),
                },
            })
        );
    }
}

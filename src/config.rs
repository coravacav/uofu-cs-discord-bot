use anyhow::Context;
use chrono::Duration;
use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::sync::Arc;

use crate::lang::Ruleset;
use crate::starboard::Starboard;

#[serde_as]
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct Config {
    #[serde_as(as = "serde_with::DurationSeconds<i64>")]
    #[serde(default = "get_default_text_detect_cooldown")]
    pub default_text_detect_cooldown: Duration,
    pub starboards: Vec<Starboard>,
    pub bot_react_role_id: u64,
    pub responses: Vec<MessageResponse>,
    /// How often kingfisher replies to a message.
    pub default_hit_rate: f64,

    #[serde(skip)]
    pub config_path: String,
}

impl Config {
    /// Fetches the config from the config file in the root directory.
    pub fn create_from_file(config_path: &str) -> anyhow::Result<Config> {
        let file = std::fs::read_to_string(config_path).context("Could not read config file")?;

        let config = toml::from_str(&file).context("Could not parse config file")?;

        Ok(Config {
            config_path: config_path.to_owned(),
            ..config
        })
    }

    /// Reloads the config file and updates the configuration.
    pub fn reload(&mut self) {
        if let Ok(config) = Config::create_from_file(&self.config_path) {
            *self = config;
        }
    }

    /// Updates config with the new cooldown, and updates the cooldown as well
    pub fn update_cooldown(&mut self, cooldown: Duration) {
        self.default_text_detect_cooldown = cooldown;

        self.save();
    }

    /// Adds a response to the config file and the config.
    pub fn add_response(&mut self, response: MessageResponse) {
        self.responses.push(response);
        self.save();
    }

    /// Removes a response from the config file and the config.
    pub fn remove_response(&mut self, name: String) {
        self.responses.retain(|response| *response.name != name);
        self.save();
    }

    pub fn get_response(&self, name: &str) -> &MessageResponseKind {
        &self
            .responses
            .iter()
            .find(|response| *response.name == name)
            .expect("Could not find response with name")
            .message_response
    }

    pub fn save(&self) {
        let toml = toml::to_string(&self).expect("Could not serialize config");

        std::fs::write(&self.config_path, toml).expect("Could not write to config");
    }
}

fn get_default_text_detect_cooldown() -> Duration {
    Duration::seconds(45)
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(untagged)]
pub enum MessageResponseKind {
    #[default]
    None,
    Text {
        content: String,
    },
    RandomText {
        content: Vec<String>,
    },
    Image {
        path: String,
    },
    TextAndImage {
        content: String,
        path: String,
    },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
pub struct MessageResponse {
    name: Arc<String>,
    hit_rate: Option<f64>,
    ruleset: Ruleset,
    #[serde(flatten)]
    /// This makes it so it pretends the attributes of the enum are attributes of the struct
    message_response: Arc<MessageResponseKind>,

    /// This is not serialized, and is instead set to the current time when the config is loaded.
    #[serde(skip)]
    #[serde(default = "default_time")]
    last_triggered: DateTime<Utc>,

    /// Cooldown in seconds
    cooldown: Option<i64>,
}

fn default_time() -> DateTime<Utc> {
    DateTime::<Utc>::MIN_UTC
}

impl MessageResponse {
    pub fn is_valid_response(
        &mut self,
        input: &str,
        default_duration: Duration,
        default_hit_rate: f64,
    ) -> Option<Arc<MessageResponseKind>> {
        let duration = self
            .cooldown
            .map(Duration::seconds)
            .unwrap_or(default_duration);

        if self.ruleset.matches(input) {
            if self.last_triggered <= Utc::now() - duration {
                let hit_rate = self.hit_rate.unwrap_or(default_hit_rate);

                if rand::random::<f64>() > hit_rate {
                    println!("{} `{}`", "Miss".red(), self.name);
                    return None;
                }

                println!("{} `{}`", "Hit".green(), self.name);

                self.last_triggered = Utc::now();
                Some(Arc::clone(&self.message_response))
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{fast_ruleset, starboard::EmoteType};

    use super::*;

    #[test]
    fn should_deserialize_properly() {
        let test_input = r#"
bot_react_role_id = 123456789109876
default_hit_rate = 1.0

[[starboards]]
reaction_count = 3
emote_name = "star"
channel_id = 123456789109876

[[responses]]
name = "1984"
ruleset = '''
r 1234
!r 4312
'''
content = "literally 1984""#;

        let config: Config = toml::from_str(test_input).unwrap();

        assert_eq!(
            config,
            Config {
                default_text_detect_cooldown: Duration::seconds(45),
                starboards: vec![Starboard {
                    reaction_count: 3,
                    emote_type: EmoteType::CustomEmote {
                        emote_name: "star".to_owned()
                    },
                    channel_id: 123456789109876,
                    ..Default::default()
                }],
                default_hit_rate: 1.,
                bot_react_role_id: 123456789109876,
                responses: vec![MessageResponse {
                    name: Arc::new("1984".to_owned()),
                    hit_rate: None,
                    ruleset: fast_ruleset!("r 1234\n!r 4312"),
                    message_response: Arc::new(MessageResponseKind::Text {
                        content: "literally 1984".to_owned()
                    }),
                    last_triggered: DateTime::<Utc>::MIN_UTC,
                    cooldown: None,
                }],
                config_path: "".to_owned(),
            }
        );
    }
}

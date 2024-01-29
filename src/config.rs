use crate::lang::ruleset::Ruleset;
use crate::starboard::Starboard;
use chrono::{DateTime, Utc};
use chrono::{Duration, Local};
use color_eyre::eyre::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::sync::Arc;

#[serde_as]
#[derive(Deserialize, Serialize, PartialEq, Debug)]
pub struct Config {
    /// The default cooldown for text detection.
    ///
    /// This can be overridden by the `cooldown` field in a response.
    #[serde_as(as = "serde_with::DurationSeconds<i64>")]
    #[serde(default = "get_default_text_detect_cooldown")]
    pub default_text_detect_cooldown: Duration,
    /// The starboards that kingfisher will listen for / update.
    pub starboards: Vec<Starboard>,
    /// The id of the guild the bot is in.
    pub guild_id: u64,
    /// The role id of the bot react role.
    pub bot_react_role_id: u64,
    /// What possible replies kingfisher can make.
    pub responses: Vec<RegisteredResponse>,
    /// How often kingfisher replies to a message.
    pub default_hit_rate: f64,
    /// The path to the config file.
    /// This is to allow for saving / reloading the config.
    #[serde(skip)]
    pub config_path: String,
}

impl Config {
    /// Fetches the config from the config file in the root directory.
    pub fn create_from_file(config_path: &str) -> Result<Config> {
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

    pub fn save(&self) -> Result<()> {
        let toml = toml::to_string(&self).context("Could not serialize config")?;

        std::fs::write(&self.config_path, toml).context("Could not save config")
    }
}

fn get_default_text_detect_cooldown() -> Duration {
    Duration::seconds(45)
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(untagged)]
pub enum ResponseKind {
    /// There is no response.
    #[default]
    None,
    /// A text response.
    Text { content: String },
    /// A random text response.
    RandomText { content: Vec<String> },
    /// An image response.
    Image { path: String },
    /// A text and image response.
    TextAndImage { content: String, path: String },
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
pub struct RegisteredResponse {
    /// The name of the response. Used only for logging.
    name: Arc<String>,
    /// The chance that the response will be triggered.
    ///
    /// Overrides the default hit rate.
    hit_rate: Option<f64>,
    /// Under what rules the response should be triggered.
    ruleset: Ruleset,
    /// This makes it so it pretends the attributes of the enum are attributes of the struct
    #[serde(flatten)]
    message_response: Arc<ResponseKind>,
    /// Per response storage of when the response was last triggered.
    #[serde(skip)]
    #[serde(default = "default_time")]
    last_triggered: DateTime<Utc>,
    /// Cooldown in seconds.
    ///
    /// Overrides the default cooldown.
    cooldown: Option<i64>,
}

fn default_time() -> DateTime<Utc> {
    DateTime::<Utc>::MIN_UTC
}

impl RegisteredResponse {
    pub fn is_valid_response(
        &mut self,
        input: &str,
        default_duration: Duration,
        default_hit_rate: f64,
    ) -> Option<Arc<ResponseKind>> {
        let duration = self
            .cooldown
            .map(Duration::seconds)
            .unwrap_or(default_duration);

        if self.ruleset.matches(input) {
            if self.last_triggered <= Utc::now() - duration {
                let hit_rate = self.hit_rate.unwrap_or(default_hit_rate);

                let now = Local::now().format("%Y-%m-%d %H:%M:%S");

                if rand::random::<f64>() > hit_rate {
                    println!("{now} {} `{}`", "Miss".red(), self.name);
                    return None;
                }

                println!("{now} {} `{}`", "Hit ".green(), self.name);

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
guild_id = 123456789109876

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
                guild_id: 123456789109876,
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
                responses: vec![RegisteredResponse {
                    name: Arc::new("1984".to_owned()),
                    hit_rate: None,
                    ruleset: fast_ruleset!("r 1234\n!r 4312"),
                    message_response: Arc::new(ResponseKind::Text {
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

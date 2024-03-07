use crate::lang::ruleset::Ruleset;
use crate::starboard::Starboard;
use chrono::{DateTime, Utc};
use chrono::{Duration, Local};
use color_eyre::eyre::{Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationSeconds};
use std::sync::Arc;
use tracing::{event, Level};

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, Clone)]
pub struct ReactRole {
    pub react: bool,
    pub user_id: u64,
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    /// The default cooldown for text detection.
    ///
    /// This can be overridden by the `cooldown` field in a response.
    #[serde_as(as = "serde_with::DurationSeconds<i64>")]
    #[serde(default = "get_default_text_detect_cooldown")]
    pub default_text_detect_cooldown: Duration,
    /// The starboards that kingfisher will listen for / update.
    pub starboards: Vec<Arc<Starboard>>,
    /// The id of the guild the bot is in.
    pub guild_id: u64,
    /// The role id of the bot react role.
    pub bot_react_role_id: u64,
    /// What possible replies kingfisher can make.
    pub responses: Vec<RegisteredResponse>,
    /// How often kingfisher replies to a message.
    pub default_hit_rate: f64,
    /// Verbatim text to skip the hit rate check.
    /// Intentionally only a single string to prevent having to check a lot of different strings.
    pub skip_hit_rate_text: String,
    /// Verbatim text to skip the duration check.
    /// Intentionally only a single string to prevent having to check a lot of different strings.
    pub skip_duration_text: String,
    /// The path to the config file.
    /// This is to allow for saving / reloading the config.
    #[serde(skip)]
    pub config_path: String,
    /// Our own cache of members with the bot react role.
    /// This may be rate limiting us, so we cache it.
    #[serde(skip)]
    pub bot_react_role_members: Vec<ReactRole>,
}

impl PartialEq for Config {
    fn eq(&self, other: &Self) -> bool {
        self.default_text_detect_cooldown == other.default_text_detect_cooldown
            && self.starboards == other.starboards
            && self.guild_id == other.guild_id
            && self.bot_react_role_id == other.bot_react_role_id
            && self.responses == other.responses
            && self.default_hit_rate == other.default_hit_rate
            && self.skip_hit_rate_text == other.skip_hit_rate_text
            && self.config_path == other.config_path
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            default_text_detect_cooldown: get_default_text_detect_cooldown(),
            starboards: vec![],
            guild_id: 0,
            skip_duration_text: "".to_owned(),
            bot_react_role_id: 0,
            responses: vec![],
            default_hit_rate: 1.,
            skip_hit_rate_text: "".to_owned(),
            config_path: "".to_owned(),
            bot_react_role_members: vec![],
        }
    }
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

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Default)]
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
    last_triggered: Mutex<DateTime<Utc>>,
    /// Cooldown in seconds.
    ///
    /// Overrides the default cooldown.
    #[serde_as(as = "Option<DurationSeconds<i64>>")]
    cooldown: Option<Duration>,
    /// Whether or not the response can be skipped via the `skip_hit_rate_text` config option.
    #[serde(default)]
    unskippable: bool,
}

impl PartialEq for RegisteredResponse {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.hit_rate == other.hit_rate
            && self.ruleset == other.ruleset
            && self.message_response == other.message_response
            && self.cooldown == other.cooldown
    }
}

fn default_time() -> Mutex<DateTime<Utc>> {
    Mutex::new(DateTime::<Utc>::MIN_UTC)
}

impl RegisteredResponse {
    pub fn is_valid_response(
        &self,
        input: &str,
        Config {
            default_text_detect_cooldown: global_cooldown,
            skip_hit_rate_text,
            default_hit_rate: global_hit_rate,
            skip_duration_text,
            ..
        }: &Config,
        message_link: &str,
    ) -> Option<Arc<ResponseKind>> {
        if self.ruleset.matches(input) {
            let mut last_triggered = self.last_triggered.lock();
            let duration = self.cooldown.unwrap_or(*global_cooldown);

            if *last_triggered <= Utc::now() - duration || input.contains(skip_duration_text) {
                let hit_rate = self.hit_rate.unwrap_or(*global_hit_rate);

                let now = Local::now().format("%Y-%m-%d %H:%M:%S");
                let miss = rand::random::<f64>() > hit_rate;

                if miss && (self.unskippable || !input.contains(skip_hit_rate_text)) {
                    event!(Level::INFO, "Miss `{}` {} {}", self.name, message_link, now);
                    return None;
                }

                event!(Level::INFO, "Hit `{}` {} {}", self.name, message_link, now);

                *last_triggered = Utc::now();
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
skip_hit_rate_text = "kf please"

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
                starboards: vec![Arc::new(Starboard {
                    reaction_count: 3,
                    emote_type: EmoteType::CustomEmote {
                        emote_name: "star".to_owned()
                    },
                    channel_id: 123456789109876,
                    ..Default::default()
                })],
                bot_react_role_id: 123456789109876,
                responses: vec![RegisteredResponse {
                    name: Arc::new("1984".to_owned()),
                    hit_rate: None,
                    ruleset: fast_ruleset!("r 1234\n!r 4312"),
                    message_response: Arc::new(ResponseKind::Text {
                        content: "literally 1984".to_owned()
                    }),
                    last_triggered: Mutex::new(DateTime::<Utc>::MIN_UTC),
                    cooldown: None,
                    unskippable: false,
                }],
                skip_hit_rate_text: "kf please".to_owned(),
                ..Default::default()
            }
        );
    }
}

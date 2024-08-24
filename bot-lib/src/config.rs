use crate::lang::ruleset::Ruleset;
use crate::starboard::Starboard;
use chrono::{DateTime, Utc};
use chrono::{Duration, Local};
use color_eyre::eyre::{Result, WrapErr};
use parking_lot::Mutex;
use poise::serenity_prelude::ChannelId;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationSeconds};
use std::path::Path;
use std::sync::Arc;

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
    /// The help text for the bot. `/help`
    pub help_text: Option<Arc<String>>,
    /// The role id of the bot react role.
    pub bot_react_role_id: u64,
    /// The role id of the woof react role.
    pub dog_react_role_id: u64,
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
    /// The list of class categories we currently support
    pub class_categories: Vec<ChannelId>,
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
            && self.class_categories == other.class_categories
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            default_text_detect_cooldown: get_default_text_detect_cooldown(),
            starboards: vec![],
            guild_id: 0,
            dog_react_role_id: 0,
            skip_duration_text: "".to_owned(),
            help_text: None,
            bot_react_role_id: 0,
            responses: vec![],
            default_hit_rate: 1.,
            skip_hit_rate_text: "".to_owned(),
            class_categories: vec![],
        }
    }
}

impl Config {
    /// Fetches the config from the config file in the root directory.
    pub fn create_from_file(config_path: impl AsRef<Path>) -> Result<Config> {
        let file = std::fs::read_to_string(config_path).wrap_err("Could not read config file")?;

        toml::from_str(&file).wrap_err("Could not parse config file")
    }

    /// Reloads the config file and updates the configuration.
    pub fn reload(&mut self, config_path: impl AsRef<Path>) {
        if let Ok(config) = Config::create_from_file(config_path) {
            *self = config;
        }
    }

    pub fn save(&self, config_path: impl AsRef<Path>) -> Result<()> {
        let toml = toml::to_string(&self).wrap_err("Could not serialize config")?;

        std::fs::write(config_path, toml).wrap_err("Could not save config")
    }
}

const fn get_default_text_detect_cooldown() -> Duration {
    match chrono::TimeDelta::try_seconds(45) {
        Some(duration) => duration,
        None => panic!("Could not create default text detect cooldown"),
    }
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
#[derive(Deserialize, Serialize, Debug)]
pub struct RegisteredResponse {
    /// The name of the response. Used only for logging.
    name: Arc<str>,
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
    DateTime::<Utc>::MIN_UTC.into()
}

impl RegisteredResponse {
    pub fn find_valid_response(
        &self,
        input: &str,
        Config {
            default_text_detect_cooldown: global_cooldown,
            skip_hit_rate_text,
            default_hit_rate,
            skip_duration_text,
            ..
        }: &Config,
        message_link: &str,
    ) -> Option<Arc<ResponseKind>> {
        if !self.ruleset.matches(input) {
            return None;
        }

        let mut last_triggered = self.last_triggered.lock();
        let cooldown = self.cooldown.unwrap_or(*global_cooldown);
        let time_since_last_triggered = Utc::now() - *last_triggered;
        let allowed = time_since_last_triggered > cooldown;
        let blocked = !input.contains(skip_duration_text);

        if !allowed && blocked {
            tracing::debug!(
                "Cooldown `{}` {} remaining {}",
                self.name,
                message_link,
                cooldown - time_since_last_triggered
            );

            return None;
        }

        let now = Local::now().format("%Y-%m-%d %H:%M:%S");
        let hit_rate = self.hit_rate.unwrap_or(*default_hit_rate);
        let miss = rand::random::<f64>() > hit_rate;
        let blocked = self.unskippable || !input.contains(skip_hit_rate_text);

        if miss && blocked {
            tracing::debug!("Miss `{}` {} {}", self.name, message_link, now);
            return None;
        }

        tracing::debug!("Hit `{}` {} {}", self.name, message_link, now);

        *last_triggered = Utc::now();

        Some(Arc::clone(&self.message_response))
    }
}

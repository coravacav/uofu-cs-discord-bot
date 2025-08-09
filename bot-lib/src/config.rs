use crate::lang::ruleset_combinator::RulesetCombinator;
use crate::starboard::Starboard;
use chrono::Duration;
use chrono::{DateTime, TimeDelta, Utc};
use color_eyre::eyre::{Result, WrapErr};
use parking_lot::Mutex;
use poise::serenity_prelude::ChannelId;
use rand::prelude::*;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use serde_with::{DurationSeconds, serde_as};
use std::path::Path;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct Ids {
    /// The role id of the bot react role.
    pub bot_react_role_id: u64,
    /// The role id of the woof react role.
    pub dog_react_role_id: u64,
}

/// This is the raw config file that's read from the config file (config.toml)
#[derive(Deserialize)]
pub struct RawConfig {
    pub default_text_detect_cooldown: u64,
    pub starboards: Vec<Arc<Starboard>>,
    #[serde(flatten)]
    pub ids: Ids,
    pub help_text: Option<Arc<String>>,
    pub responses: Vec<RawRegisteredResponse>,
    pub default_hit_rate: f64,
    pub skip_hit_rate_text: String,
    pub skip_duration_text: String,
    pub class_categories: Vec<ChannelId>,
}

impl RawConfig {
    /// Fetches the config from the config file in the root directory.
    pub fn create_from_file(config_path: impl AsRef<Path>) -> Result<Self> {
        let file = std::fs::read_to_string(config_path).wrap_err("Could not read config file")?;

        toml::from_str(&file).wrap_err("Could not parse config file")
    }
}

pub struct Config {
    /// The default cooldown for text detection.
    ///
    /// This can be overridden by the `cooldown` field in a response.
    pub default_text_detect_cooldown: TimeDelta,
    /// The starboards that kingfisher will listen for / update.
    pub starboards: Vec<Arc<Starboard>>,
    /// Contains special ids
    pub ids: Ids,
    /// The help text for the bot. `/help`
    pub help_text: Option<Arc<String>>,
    /// What possible replies kingfisher can make.
    pub responses: FxHashMap<Arc<str>, AutomatedKingfisherReplyConfig>,
    /// The ruleset combinator
    pub ruleset_combinator: RulesetCombinator,
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

impl Config {
    pub fn new(raw_config: RawConfig) -> Result<Self> {
        let unparsed_rulesets = raw_config
            .responses
            .iter()
            .map(|response| response.unparsed_ruleset.as_str().try_into())
            .collect::<Result<Vec<_>>>()?;

        let ruleset_combinator = RulesetCombinator::new(
            raw_config
                .responses
                .iter()
                .map(|response| response.name.clone())
                .zip(unparsed_rulesets.into_iter())
                .map(|a| a.into()),
        )?;

        let responses = FxHashMap::from_iter(raw_config.responses.into_iter().map(|response| {
            (
                response.name.clone(),
                AutomatedKingfisherReplyConfig {
                    hit_rate: response.hit_rate.unwrap_or(raw_config.default_hit_rate),
                    message_response: response.message_response,
                    last_triggered: Mutex::new(DateTime::UNIX_EPOCH),
                    cooldown: response.cooldown,
                    unskippable: response.unskippable,
                },
            )
        }));

        let default_text_detect_cooldown: i64 =
            raw_config.default_text_detect_cooldown.try_into()?;

        Ok(Self {
            default_text_detect_cooldown: TimeDelta::seconds(default_text_detect_cooldown),
            starboards: raw_config.starboards,
            skip_duration_text: raw_config.skip_duration_text,
            help_text: raw_config.help_text,
            responses,
            ruleset_combinator,
            default_hit_rate: raw_config.default_hit_rate,
            skip_hit_rate_text: raw_config.skip_hit_rate_text,
            class_categories: raw_config.class_categories,
            ids: raw_config.ids,
        })
    }

    /// Fetches the config from the config file in the root directory.
    pub fn create_from_file(config_path: impl AsRef<Path>) -> Result<Config> {
        let file = std::fs::read_to_string(config_path).wrap_err("Could not read config file")?;

        let raw_config = toml::from_str(&file).wrap_err("Could not parse config file")?;

        Self::new(raw_config)
    }

    /// Reloads the config file and updates the configuration.
    pub fn reload(&mut self, config_path: impl AsRef<Path>) {
        if let Ok(config) = Config::create_from_file(config_path) {
            *self = config;
        }
    }
}

/// All different ways for a message detection to reply.
///
/// Future plans include something like images or embeds, but, that's not implemented yet.
#[derive(Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(untagged)]
pub enum ResponseKind {
    /// There is no response.
    #[default]
    None,
    /// A text response.
    Text { content: Arc<str> },
    /// A random text response.
    RandomText { content: Vec<Arc<str>> },
}

impl ResponseKind {
    pub fn get_reply_text(&self) -> Option<Arc<str>> {
        match self {
            ResponseKind::Text { content } => Some(content.clone()),
            ResponseKind::RandomText { content } => {
                content.choose(&mut rand::rng()).map(Arc::clone)
            }
            ResponseKind::None => None,
        }
    }
}

/// A nice little configuration object containing all individually configurable message settings and responses.
pub struct AutomatedKingfisherReplyConfig {
    /// The chance that the response will be triggered.
    ///
    /// This is set to the global hit rate if not set.
    hit_rate: f64,
    /// This makes it so it pretends the attributes of the enum are attributes of the struct
    message_response: Arc<ResponseKind>,
    /// Per response storage of when the response was last triggered.
    last_triggered: Mutex<DateTime<Utc>>,
    /// Cooldown in seconds.
    ///
    /// Overrides the default cooldown.
    cooldown: Option<Duration>,
    /// Whether or not the response can be skipped via the `skip_hit_rate_text` config option.
    unskippable: bool,
}

#[serde_as]
#[derive(Deserialize)]
pub struct RawRegisteredResponse {
    /// The name of the response. Used only for logging.
    name: Arc<str>,
    /// The chance that the response will be triggered.
    ///
    /// Overrides the default hit rate.
    hit_rate: Option<f64>,
    /// Under what rules the response should be triggered.
    #[serde(rename = "ruleset")]
    unparsed_ruleset: String,
    /// This makes it so it pretends the attributes of the enum are attributes of the struct
    #[serde(flatten)]
    message_response: Arc<ResponseKind>,
    /// Cooldown in seconds.
    ///
    /// Overrides the default cooldown.
    #[serde_as(as = "Option<DurationSeconds<i64>>")]
    cooldown: Option<Duration>,
    /// Whether or not the response can be skipped via the `skip_hit_rate_text` config option.
    #[serde(default)]
    unskippable: bool,
}

impl AutomatedKingfisherReplyConfig {
    pub fn can_send(
        &self,
        input: &str,
        Config {
            default_text_detect_cooldown: global_cooldown,
            skip_hit_rate_text,
            skip_duration_text,
            ..
        }: &Config,
    ) -> Option<Arc<ResponseKind>> {
        let mut last_triggered = self.last_triggered.lock();
        let cooldown = self.cooldown.unwrap_or(*global_cooldown);
        let time_since_last_triggered = Utc::now() - *last_triggered;
        let allowed = time_since_last_triggered > cooldown;
        let blocked = !input.contains(skip_duration_text);

        if !allowed && blocked {
            return None;
        }

        let hit_rate = self.hit_rate;
        let miss = rand::random::<f64>() > hit_rate;
        let blocked = self.unskippable || !input.contains(skip_hit_rate_text);

        if miss && blocked {
            return None;
        }

        *last_triggered = Utc::now();

        Some(Arc::clone(&self.message_response))
    }
}

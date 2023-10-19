use chrono::Duration;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, MutexGuard};

/// In minutes
const DEFAULT_TEXT_DETECT_COOLDOWN: i64 = 5;

pub struct Config {
    text_detect_cooldown: Mutex<Duration>,
    discord_token: String,
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

        // this is just a shortcut to save the file lol
        let config = Config {
            text_detect_cooldown: Mutex::new(text_detect_cooldown),
            discord_token,
        };
        config.update_cooldown(text_detect_cooldown);

        return config;
    }

    /// Updates config.toml with the new cooldown, and updates the cooldown as well
    pub fn update_cooldown(&self, cooldown: Duration) {
        let mut text_detect_cooldown = self.lock_cooldown();
        *text_detect_cooldown = cooldown;

        let config_builder = ConfigBuilder {
            text_detect_cooldown: Some(cooldown.num_minutes()),
            discord_token: Some(self.discord_token.clone()),
        };
        let toml = toml::to_string(&config_builder).unwrap();

        std::fs::write("./config.toml", toml).expect("Could not write to config.toml");
    }

    /// Locks and returns the text detect cooldown.
    /// The mutex guard returned is guaranteed to be unlocked, so can be used immediately.
    pub fn lock_cooldown(&self) -> MutexGuard<Duration> {
        let text_detect_cooldown = self
            .text_detect_cooldown
            .lock()
            .expect("Could not lock mutex");
        return text_detect_cooldown;
    }

    pub fn get_token(&self) -> String {
        return self.discord_token.clone();
    }
}

#[derive(Deserialize, Serialize)]
struct ConfigBuilder {
    text_detect_cooldown: Option<i64>,
    discord_token: Option<String>,
}

impl ConfigBuilder {
    fn empty() -> ConfigBuilder {
        ConfigBuilder {
            text_detect_cooldown: None,
            discord_token: None,
        }
    }
}

use std::{path::Path, sync::Arc};

use crate::config::{Config, MessageResponse, MessageResponseKind};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use notify::Watcher;
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::Message;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use tokio::sync::RwLock;

pub struct Data {
    last_responses: DashMap<Arc<String>, DateTime<Utc>>,
    pub config: Arc<RwLock<Config>>,
}

impl Data {
    pub fn init(config: Config) -> Data {
        let config_path = config.get_config_path().to_owned();
        let config_path = Path::new(&config_path);

        let last_responses = config
            .get_responses()
            .iter()
            .map(|response| (Arc::clone(&response.name), DateTime::<Utc>::UNIX_EPOCH))
            .collect();

        let config = Arc::new(RwLock::new(config));

        let data = Data {
            last_responses,
            config,
        };

        let config_clone = Arc::clone(&data.config);

        let mut watcher = notify::recommended_watcher(move |res| match res {
            Ok(_) => {
                println!("config.toml changed, reloading...");
                config_clone.blocking_write().reload();
            }
            Err(e) => println!("watch error: {:?}", e),
        })
        .expect("Failed to create file watcher");

        watcher
            .watch(config_path, notify::RecursiveMode::NonRecursive)
            .expect(format!("Failed to watch {:?}", config_path).as_str());

        data
    }

    /// Register a new response type for messages matching a regular expression pattern
    pub fn register(&mut self, response: MessageResponse) {
        self.last_responses
            .insert(Arc::clone(&response.name), DateTime::<Utc>::UNIX_EPOCH);

        self.config.blocking_write().add_response(response);
    }

    /// Reload the configuration file and update the responses hash map accordingly
    pub fn reload(&self) {
        self.config.blocking_write().reload();
        self.config
            .blocking_write()
            .get_responses()
            .iter()
            .for_each(|response| {
                self.reset_last_response(Arc::clone(&response.name), DateTime::<Utc>::UNIX_EPOCH)
            });
    }

    /// If the message contents match any pattern, return the name of the response type.
    /// Otherwise, return None
    pub async fn check_should_respond<'a>(&'a self, message: &Message) -> Option<Arc<String>> {
        self.config
            .read()
            .await
            .get_responses()
            .par_iter()
            .find_map_any(|response| {
                if response.ruleset.matches(&message.content) {
                    Some(Arc::clone(&response.name))
                } else {
                    None
                }
            })
    }

    pub fn last_response(&self, name: Arc<String>) -> Option<DateTime<Utc>> {
        self.last_responses.get(&name).as_deref().copied()
    }

    pub fn reset_last_response(&self, name: Arc<String>, timestamp: DateTime<Utc>) {
        self.last_responses
            .entry(name)
            .and_modify(|v| *v = timestamp)
            .or_insert(timestamp);
    }

    pub async fn run_action(
        &self,
        name: &str,
        message: &Message,
        ctx: &serenity::Context,
    ) -> anyhow::Result<()> {
        let config = self.config.read().await;
        let action = config.get_response(name);
        match &action.kind {
            MessageResponseKind::Text { content, .. } => {
                message.reply(ctx, content).await?;
            }
            MessageResponseKind::RandomText { content, .. } => {
                let pick_index = rand::random::<usize>() % content.len();
                message.reply(ctx, &content[pick_index]).await?;
            }
            MessageResponseKind::Image { path, .. } => {
                message
                    .channel_id
                    .send_message(ctx, |m| {
                        m.reference_message(message);
                        m.allowed_mentions(|am| am.replied_user(false));
                        m.add_file(path.as_str())
                    })
                    .await?;
            }
            MessageResponseKind::TextAndImage { content, path, .. } => {
                message
                    .channel_id
                    .send_message(ctx, |m| {
                        m.reference_message(message);
                        m.allowed_mentions(|am| am.replied_user(false));
                        m.content(content);
                        m.add_file(path.as_str())
                    })
                    .await?;
            }
        }
        Ok(())
    }
}

// User data, which is stored and accessible in all command invocations
pub type PoiseContext<'a> = poise::Context<'a, Data, anyhow::Error>;

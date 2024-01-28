use crate::config::{Config, MessageResponse, MessageResponseKind};
use color_eyre::eyre::{Error, Result};
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::Message;
use rand::seq::SliceRandom;
use std::{path::Path, sync::Arc};
use tokio::sync::RwLock;

pub struct Data {
    pub config: Arc<RwLock<Config>>,
}

impl Data {
    pub fn new(config: Config) -> Data {
        let config = Arc::new(RwLock::new(config));

        let data = Data { config };

        data.setup_file_watcher();

        data
    }

    fn setup_file_watcher(&self) {
        let config_clone = Arc::clone(&self.config);

        std::thread::spawn(move || {
            let config_path = config_clone.blocking_read().config_path.to_owned();

            use notify::{
                event::{AccessKind, AccessMode},
                Event, EventKind, RecursiveMode, Watcher,
            };

            let mut watcher = notify::recommended_watcher(move |res| match res {
                Ok(Event {
                    kind: EventKind::Access(AccessKind::Close(AccessMode::Write)),
                    ..
                }) => {
                    println!("config changed, reloading...");

                    Arc::clone(&config_clone).blocking_write().reload();
                }
                Err(e) => println!("watch error: {:?}", e),
                _ => {}
            })
            .expect("Failed to create file watcher");

            watcher
                .watch(Path::new(&config_path), RecursiveMode::NonRecursive)
                .expect("Failed to watch config file");

            // Sleep thread to keep watcher alive
            loop {
                std::thread::sleep(std::time::Duration::MAX);
            }
        });
    }

    /// Register a new response type for messages matching a regular expression pattern
    pub fn register(&mut self, response: MessageResponse) {
        self.config.blocking_write().add_response(response);
    }

    /// Reload the configuration file and update the responses hash map accordingly
    pub fn reload(&self) {
        self.config.blocking_write().reload();
    }

    /// If the message contents match any pattern, return the name of the response type.
    /// Otherwise, return None
    pub async fn find_response<'a>(&'a self, message: &str) -> Option<Arc<MessageResponseKind>> {
        let mut config = self.config.write().await;
        let global_cooldown = config.default_text_detect_cooldown;
        let default_hit_rate = config.default_hit_rate;

        config.responses.iter_mut().find_map(|response| {
            response.is_valid_response(message, global_cooldown, default_hit_rate)
        })
    }

    pub async fn run_action(
        &self,
        message_response: &MessageResponseKind,
        reply_target: &Message,
        ctx: &serenity::Context,
    ) -> Result<()> {
        match message_response {
            MessageResponseKind::Text { content } => {
                reply_target.reply(ctx, content).await?;
            }
            MessageResponseKind::RandomText { content } => {
                let response = {
                    let mut rng = rand::thread_rng();
                    content
                        .choose(&mut rng)
                        .expect("The responses list is empty")
                };

                reply_target.reply(ctx, response).await?;
            }
            MessageResponseKind::Image { path } => {
                reply_target
                    .channel_id
                    .send_message(
                        ctx,
                        serenity::CreateMessage::new()
                            .reference_message(reply_target)
                            .allowed_mentions(
                                serenity::CreateAllowedMentions::new().replied_user(false),
                            )
                            .add_file(serenity::CreateAttachment::path(&path).await?),
                    )
                    .await?;
            }
            MessageResponseKind::TextAndImage { content, path } => {
                reply_target
                    .channel_id
                    .send_message(
                        ctx,
                        serenity::CreateMessage::new()
                            .reference_message(reply_target)
                            .allowed_mentions(
                                serenity::CreateAllowedMentions::new().replied_user(false),
                            )
                            .content(content)
                            .add_file(serenity::CreateAttachment::path(&path).await?),
                    )
                    .await?;
            }
            MessageResponseKind::None => {}
        }

        Ok(())
    }
}

// User data, which is stored and accessible in all command invocations
pub type PoiseContext<'a> = poise::Context<'a, Data, Error>;

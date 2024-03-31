use crate::config::{Config, ResponseKind};
use color_eyre::eyre::{Error, OptionExt, Result};
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::Message;
use rand::seq::SliceRandom;
use std::{path::Path, sync::Arc};
use tokio::sync::RwLock;
use tracing::{event, Level};

#[derive(Debug)]
pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    /// Config file watcher that refreshes the config if it changes
    ///
    /// Attached to the AppState to keep the watcher alive
    _watcher: notify::RecommendedWatcher,
}

impl AppState {
    pub fn new(config: Config) -> AppState {
        let config = Arc::new(RwLock::new(config));

        let config_path = config.blocking_read().config_path.to_owned();

        use notify::{
            event::{AccessKind, AccessMode},
            Event, EventKind, RecursiveMode, Watcher,
        };

        let config_clone = Arc::clone(&config);

        let mut watcher = notify::recommended_watcher(move |res| match res {
            Ok(Event {
                kind: EventKind::Access(AccessKind::Close(AccessMode::Write)),
                ..
            }) => {
                event!(Level::INFO, "config changed, reloading...");

                config_clone.blocking_write().reload();
            }
            Err(e) => event!(Level::ERROR, "watch error: {:?}", e),
            _ => {}
        })
        .expect("Failed to create file watcher");

        watcher
            .watch(Path::new(&config_path), RecursiveMode::NonRecursive)
            .expect("Failed to watch config file");

        AppState {
            config,
            _watcher: watcher,
        }
    }

    /// If the message contents match any pattern, return the name of the response type.
    /// Otherwise, return None
    pub async fn find_response(
        &self,
        message: &str,
        message_link: &str,
    ) -> Option<Arc<ResponseKind>> {
        let config = self.config.read().await;

        config
            .responses
            .iter()
            .find_map(|response| response.find_valid_response(message, &config, message_link))
    }

    pub async fn run_action(
        &self,
        message_response: &ResponseKind,
        reply_target: &Message,
        ctx: &serenity::Context,
    ) -> Result<()> {
        match message_response {
            ResponseKind::Text { content } => {
                reply_target.reply(ctx, content).await?;
            }
            ResponseKind::RandomText { content } => {
                let response = content
                    .choose(&mut rand::thread_rng())
                    .ok_or_eyre("The responses list is empty")?;

                reply_target.reply(ctx, response).await?;
            }
            ResponseKind::Image { path } => {
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
            ResponseKind::TextAndImage { content, path } => {
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
            ResponseKind::None => {}
        }

        Ok(())
    }
}

// User data, which is stored and accessible in all command invocations
pub type PoiseContext<'a> = poise::Context<'a, AppState, Error>;

use std::{path::Path, sync::Arc};

use crate::config::{Config, MessageResponse, MessageResponseKind};

use notify::Watcher;
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::Message;
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
            let config_path = config_clone.blocking_read().get_config_path().to_owned();

            let notify_config = notify::Config::default()
                .with_poll_interval(std::time::Duration::from_secs(2))
                .with_compare_contents(true);

            let mut watcher = notify::PollWatcher::new(
                move |res| match res {
                    Ok(_) => {
                        println!("config changed, reloading...");

                        Arc::clone(&config_clone).blocking_write().reload();
                    }
                    Err(e) => println!("watch error: {:?}", e),
                },
                notify_config,
            )
            .expect("Failed to create file watcher");

            watcher
                .watch(Path::new(&config_path), notify::RecursiveMode::NonRecursive)
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
        self.config
            .write()
            .await
            .responses
            .iter_mut()
            .find_map(|response| response.is_valid_response(message))
    }

    pub async fn run_action(
        &self,
        message_response: &MessageResponseKind,
        reply_target: &Message,
        ctx: &serenity::Context,
    ) -> anyhow::Result<()> {
        match message_response {
            MessageResponseKind::Text { content, .. } => {
                reply_target.reply(ctx, content).await?;
            }
            MessageResponseKind::RandomText { content, .. } => {
                let pick_index = rand::random::<usize>() % content.len();
                reply_target.reply(ctx, &content[pick_index]).await?;
            }
            MessageResponseKind::Image { path, .. } => {
                reply_target
                    .channel_id
                    .send_message(ctx, |m| {
                        m.reference_message(reply_target);
                        m.allowed_mentions(|am| am.replied_user(false));
                        m.add_file(path.as_str())
                    })
                    .await?;
            }
            MessageResponseKind::TextAndImage { content, path, .. } => {
                reply_target
                    .channel_id
                    .send_message(ctx, |m| {
                        m.reference_message(reply_target);
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

use crate::config::{Config, MessageResponse, MessageResponseKind};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::Message;

pub struct Data {
    last_responses: DashMap<String, DateTime<Utc>>,
    pub config: Config,
}

impl Data {
    pub fn init(config: Config) -> Data {
        let last_responses = config
            .get_responses()
            .iter()
            .map(|response| (response.name.clone(), DateTime::<Utc>::UNIX_EPOCH))
            .collect();

        Data {
            last_responses,
            config,
        }
    }

    /// Register a new response type for messages matching a regular expression pattern
    pub fn register(&mut self, response: MessageResponse) {
        self.last_responses
            .insert(response.name.clone(), DateTime::<Utc>::UNIX_EPOCH);

        self.config.add_response(response);
    }

    /// Reload the configuration file and update the responses hash map accordingly
    pub fn reload(&self) {
        self.config.reload();
        self.last_responses
            .alter_all(|_, _| DateTime::<Utc>::UNIX_EPOCH);
    }

    /// If the message contents match any pattern, return the name of the response type.
    /// Otherwise, return None
    pub fn check_should_respond(&self, message: &Message) -> Option<String> {
        self.config.get_responses().iter().find_map(|response| {
            if response.get_pattern().is_match(&message.content) {
                Some(response.name.clone())
            } else {
                None
            }
        })
    }

    pub fn last_response(&self, name: &str) -> Option<DateTime<Utc>> {
        self.last_responses.get(name).as_deref().copied()
    }

    pub fn reset_last_response(&self, name: &str, timestamp: DateTime<Utc>) {
        self.last_responses
            .entry(name.to_owned())
            .and_modify(|v| *v = timestamp)
            .or_insert(timestamp);
    }

    pub async fn run_action(
        &self,
        name: &str,
        message: &Message,
        ctx: &serenity::Context,
    ) -> anyhow::Result<()> {
        let action = self.config.get_response(name.to_string());
        match &action.kind {
            MessageResponseKind::Text { content, .. } => {
                message.reply(ctx, content).await?;
            }
            MessageResponseKind::RandomText { content, .. } => {
                let pick_index = rand::random::<usize>() % content.len();
                message.reply(ctx, content[pick_index].clone()).await?;
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

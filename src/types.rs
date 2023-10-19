use crate::config::{Config, MessageResponse};

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::Message;
use regex::Regex;

pub struct Data {
    last_responses: HashMap<String, Mutex<DateTime<Utc>>>,
    pub config: Config,
}

impl Data {
    pub fn init(config: Config) -> Data {
        let last_responses = config
            .lock_responses()
            .iter()
            .map(|response| {
                (
                    response.get_name(),
                    Mutex::new(DateTime::<Utc>::from_timestamp(0, 0).unwrap()),
                )
            })
            .collect();
        Data {
            last_responses,
            config,
        }
    }

    /// Register a new response type for messages matching a regular expression pattern
    pub fn register(&mut self, response: MessageResponse) {
        self.last_responses.insert(
            response.get_name(),
            Mutex::new(DateTime::<Utc>::from_timestamp(0, 0).unwrap()),
        );
        self.config.add_response(response);
    }

    /// If the message contents match any pattern, return the name of the response type.
    /// Otherwise, return None
    pub fn check_should_respond(&self, message: &Message) -> Option<String> {
        self.config
            .lock_responses()
            .iter()
            .find(|response| response.get_pattern().is_match(&message.content))
            .map(|response| response.get_name())
    }

    pub fn last_response(&self, name: &String) -> &Mutex<DateTime<Utc>> {
        self.last_responses.get(name).unwrap()
    }

    pub async fn run_action(
        &self,
        name: &str,
        message: &Message,
        ctx: &serenity::Context,
    ) -> Result<(), Error> {
        let action = self.config.get_response(name.to_string());
        match action {
            MessageResponse::Text { content, .. } => {
                message.reply(ctx, content).await?;
            }
            MessageResponse::RandomText { content, .. } => {
                let pick_index = rand::random::<usize>() % content.len();
                message.reply(ctx, content[pick_index].clone()).await?;
            }
            MessageResponse::Image { path, .. } => {
                message
                    .channel_id
                    .send_message(ctx, |m| {
                        m.reference_message(message);
                        m.allowed_mentions(|am| {
                            am.replied_user(false);
                            am
                        });
                        m.add_file(path.as_str());

                        m
                    })
                    .await?;
            }
            MessageResponse::TextAndImage { content, path, .. } => {
                message
                    .channel_id
                    .send_message(ctx, |m| {
                        m.reference_message(message);
                        m.allowed_mentions(|am| {
                            am.replied_user(false);
                            am
                        });
                        m.content(content);
                        m.add_file(path.as_str());

                        m
                    })
                    .await?;
            }
        }
        Ok(())
    }
}

// User data, which is stored and accessible in all command invocations
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

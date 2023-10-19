use crate::config::Config;
use crate::types::MessageAttachment::{Image, Text, TextPlusImage};

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::Message;
use regex::Regex;

pub struct Data {
    last_responses: HashMap<String, Mutex<DateTime<Utc>>>,
    response_functions: HashMap<String, MessageReply>,
    search_patterns: HashMap<String, Regex>,
    pub config: Config,
}

impl Data {
    pub fn init(config: Config) -> Data {
        Data {
            last_responses: HashMap::new(),
            response_functions: HashMap::new(),
            search_patterns: HashMap::new(),
            config,
        }
    }

    /// Register a new response type for messages matching a regular expression pattern
    pub fn register(&mut self, name: &str, pattern: &str, action: MessageReply) {
        self.search_patterns
            .insert(name.to_string(), Regex::new(pattern).unwrap());
        self.last_responses.insert(
            name.to_string(),
            Mutex::new(DateTime::<Utc>::from_timestamp(0, 0).unwrap()),
        );
        self.response_functions.insert(name.to_string(), action);
    }

    fn get_response_types(&self) -> Vec<String> {
        self.search_patterns
            .keys()
            .map(|x| x.clone())
            .collect::<Vec<String>>()
    }

    /// If the message contents match any pattern, return the name of the response type.
    /// Otherwise, return None
    pub fn check_should_respond(&self, message: &Message) -> Option<String> {
        for name in self.get_response_types() {
            if self
                .search_patterns
                .get(&name)
                .unwrap()
                .is_match(&message.content)
            {
                return Some(name);
            }
        }
        return None;
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
        let action = self.response_functions.get(name).unwrap();
        match action(message, ctx) {
            Text(text) => message.reply(ctx, text).await?,
            Image(path) => {
                message
                    .channel_id
                    .send_message(ctx, |m| {
                        m.reference_message(message);
                        m.allowed_mentions(|am| {
                            am.replied_user(false);
                            am
                        });
                        m.add_file(path);
                        return m;
                    })
                    .await?
            }
            TextPlusImage(text, path) => {
                message
                    .channel_id
                    .send_message(ctx, |m| {
                        m.reference_message(message);
                        m.allowed_mentions(|am| {
                            am.replied_user(false);
                            am
                        });
                        m.content(text);
                        m.add_file(path);
                        return m;
                    })
                    .await?
            }
        };
        Ok(())
    }
}

// User data, which is stored and accessible in all command invocations
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
pub enum MessageAttachment {
    Text(&'static str),
    Image(&'static str),
    TextPlusImage(&'static str, &'static str),
}
type MessageReply = fn(&Message, &serenity::Context) -> MessageAttachment;

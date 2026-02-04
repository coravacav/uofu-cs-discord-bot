//! Welcome to Kingfisher's internals.
//!
//! It's not all documented yet, but, damn it, it probably will never be.
//! :thum:

use chrono::{DateTime, Utc};
use color_eyre::eyre::{Context, Result};
use data::PoiseContext;
use itertools::Itertools;
use poise::serenity_prelude::{CacheHttp, EditMember, GuildId, Member, Mentionable, User, UserId};
use std::time::Duration;

pub(crate) mod automated_replies;
pub mod commands;
pub mod config;
pub(crate) mod courses;
pub mod data;
pub mod event_handler;
mod handle_starboards;
mod lang;
mod starboard;
mod text_detection;
mod utils;

pub use commands::track_message_for_limit;
pub use courses::update_course_list;
pub use starboard::debug_force_starboard;
pub use starboard::debug_surrealdb;

trait SayThenDelete {
    async fn say_then_delete(self, message: impl Into<String>) -> Result<()>;
}

impl SayThenDelete for PoiseContext<'_> {
    async fn say_then_delete(self, message: impl Into<String>) -> Result<()> {
        let message = self.say(message).await?.into_message().await?;

        let ctx = self.serenity_context().clone();

        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            message.delete(ctx).await.ok();
        });

        Ok(())
    }
}

trait TimeoutExt {
    async fn timeout(
        &self,
        cache: impl CacheHttp,
        target: &UserId,
        duration: Duration,
    ) -> Result<(Member, DateTime<Utc>)>;
}

impl TimeoutExt for GuildId {
    async fn timeout(
        &self,
        cache: impl CacheHttp,
        target: &UserId,
        duration: Duration,
    ) -> Result<(Member, DateTime<Utc>)> {
        let timeout_end = chrono::Utc::now() + duration;

        self.edit_member(
            cache,
            target,
            EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
        )
        .await
        .wrap_err("Failed to edit member")
        .map(|member| (member, timeout_end))
    }
}

trait MentionableExt {
    fn mention_all(&self) -> String;
}

impl MentionableExt for Vec<User> {
    fn mention_all(&self) -> String {
        self.iter().map(|user| user.mention().to_string()).join(" ")
    }
}
impl MentionableExt for Vec<UserId> {
    fn mention_all(&self) -> String {
        self.iter().map(|user| user.mention().to_string()).join(" ")
    }
}

impl MentionableExt for &[User] {
    fn mention_all(&self) -> String {
        self.iter().map(|user| user.mention().to_string()).join(" ")
    }
}

impl MentionableExt for &[UserId] {
    fn mention_all(&self) -> String {
        self.iter().map(|user| user.mention().to_string()).join(" ")
    }
}

use chrono::{DateTime, Utc};
use color_eyre::eyre::{Context, Result};
use data::PoiseContext;
use itertools::Itertools;
use poise::serenity_prelude::{
    Cache, CacheHttp, EditMember, GuildId, Http, Member, Mentionable, User, UserId,
};
use std::{sync::Arc, time::Duration};

pub mod commands;
pub mod config;
pub mod courses;
pub mod data;
pub mod event_handler;
mod handle_starboards;
mod lang;
pub mod llm;
mod starboard;
mod text_detection;
mod utils;

trait SayThenDelete {
    async fn say_then_delete(self, message: impl Into<String>) -> Result<()>;
}

impl<'a> SayThenDelete for PoiseContext<'a> {
    async fn say_then_delete(self, message: impl Into<String>) -> Result<()> {
        let message = self.say(message).await?;

        tokio::time::sleep(std::time::Duration::from_secs(15)).await;
        message.delete(self).await.ok();

        Ok(())
    }
}

struct CloneableCtx(Arc<Cache>, Arc<Http>);

trait IntoCloneableCtx {
    fn get_cloneable_ctx(self) -> CloneableCtx;
}

impl IntoCloneableCtx for &PoiseContext<'_> {
    fn get_cloneable_ctx(self) -> CloneableCtx {
        CloneableCtx(
            Arc::clone(&self.serenity_context().cache),
            Arc::clone(&self.serenity_context().http),
        )
    }
}

impl IntoCloneableCtx for &poise::serenity_prelude::Context {
    fn get_cloneable_ctx(self) -> CloneableCtx {
        CloneableCtx(Arc::clone(&self.cache), Arc::clone(&self.http))
    }
}

impl From<&PoiseContext<'_>> for CloneableCtx {
    fn from(ctx: &PoiseContext<'_>) -> Self {
        Self(
            Arc::clone(&ctx.serenity_context().cache),
            Arc::clone(&ctx.serenity_context().http),
        )
    }
}

impl CacheHttp for CloneableCtx {
    fn http(&self) -> &Http {
        &self.1
    }

    fn cache(&self) -> Option<&Arc<Cache>> {
        Some(&self.0)
    }
}

impl Clone for CloneableCtx {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0), Arc::clone(&self.1))
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

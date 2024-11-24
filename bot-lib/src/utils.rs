use chrono::{DateTime, Utc};
use color_eyre::eyre::Result;
use poise::CreateReply;

use crate::data::PoiseContext;

pub trait GetRelativeTimestamp {
    fn discord_relative_timestamp(&self) -> String;
}

impl GetRelativeTimestamp for DateTime<Utc> {
    fn discord_relative_timestamp(&self) -> String {
        format!("<t:{}:R>", self.timestamp())
    }
}

pub trait SendReplyEphemeral {
    async fn reply_ephemeral(&self, content: impl Into<String>) -> Result<()>;
}

impl SendReplyEphemeral for PoiseContext<'_> {
    async fn reply_ephemeral(&self, content: impl Into<String>) -> Result<()> {
        let reply = CreateReply::default()
            .reply(true)
            .ephemeral(true)
            .content(content);

        self.send(reply).await?;

        Ok(())
    }
}

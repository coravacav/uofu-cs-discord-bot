use color_eyre::eyre::Result;
use parking_lot::Mutex;
use poise::serenity_prelude::{
    Channel, ChannelId, Context, CreateAttachment, CreateEmbed, CreateEmbedAuthor, CreateMessage,
    GetMessages, Message, MessageId, ReactionType, Timestamp,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Deserialize, Serialize)]
pub struct Starboard {
    pub reaction_count: u64,
    pub channel_id: u64,
    pub ignored_channel_ids: Option<Vec<u64>>,
    /// This stores a string hash of the message link
    #[serde(skip)]
    pub recently_added_messages: Mutex<HashSet<MessageId>>,
}

impl PartialEq for Starboard {
    fn eq(&self, other: &Self) -> bool {
        self.reaction_count == other.reaction_count
            && self.channel_id == other.channel_id
            && self.ignored_channel_ids == other.ignored_channel_ids
    }
}

impl Eq for Starboard {}

impl Default for Starboard {
    fn default() -> Self {
        Self {
            reaction_count: 1,
            channel_id: 0,
            ignored_channel_ids: None,
            recently_added_messages: Mutex::new(HashSet::new()),
        }
    }
}

impl Starboard {
    #[tracing::instrument(level = "trace", skip(self, ctx, message), fields(message_link = %message.link()))]
    pub async fn does_starboard_apply(
        &self,
        ctx: &Context,
        message: &Message,
        reaction_count: u64,
    ) -> bool {
        let check = self.enough_reactions(reaction_count)
            && self.is_message_recent(&message.timestamp)
            && self.is_channel_allowed(message.channel_id.into())
            && self.is_message_unseen(&message.id)
            && self.is_message_not_yeet(message).await
            && self.is_channel_missing_reply(ctx, message).await;

        let check_msg = if check { "applies" } else { "does not apply" };
        tracing::trace!("starboard {}", check_msg);

        check
    }

    fn enough_reactions(&self, reaction_count: u64) -> bool {
        reaction_count >= self.reaction_count
    }

    fn is_message_recent(&self, message_timestamp: &Timestamp) -> bool {
        const ONE_WEEK: chrono::TimeDelta = match chrono::TimeDelta::try_weeks(1) {
            Some(time_check) => time_check,
            None => unreachable!(),
        };

        message_timestamp.unix_timestamp() > (chrono::Utc::now() - ONE_WEEK).timestamp()
    }

    fn is_channel_allowed(&self, channel_id: u64) -> bool {
        if let Some(ignored_channel_ids) = self.ignored_channel_ids.as_ref() {
            !ignored_channel_ids.contains(&channel_id)
        } else {
            true
        }
    }

    fn is_message_unseen(&self, message_id: &MessageId) -> bool {
        !self.recently_added_messages.lock().contains(message_id)
    }

    async fn is_message_not_yeet(&self, message: &Message) -> bool {
        !crate::commands::YEET_STARBOARD_EXCLUSIONS
            .lock()
            .contains(&message.id)
    }

    async fn is_channel_missing_reply(&self, ctx: &Context, message: &Message) -> bool {
        let message_link = message.link();

        let Ok(messages) = ChannelId::new(self.channel_id)
            .messages(ctx, GetMessages::new())
            .await
        else {
            return false;
        };

        let has_already_been_added = messages.iter().any(|message| {
            message.embeds.iter().any(|embed| {
                embed
                    .description
                    .as_ref()
                    .is_some_and(|description| description.contains(&message_link))
            })
        });

        !has_already_been_added
    }

    pub async fn reply(
        &self,
        ctx: &Context,
        message: &Message,
        reaction_type: &ReactionType,
    ) -> Result<()> {
        self.recently_added_messages.lock().insert(message.id);

        let reply = CreateMessage::new();

        let reply = match reaction_type {
            ReactionType::Unicode(emoji) => reply.content(emoji),
            ReactionType::Custom { animated, id, .. } => reply.add_file(
                CreateAttachment::url(
                    ctx,
                    &format!(
                        "https://cdn.discordapp.com/emojis/{}.{}",
                        id,
                        if *animated { "gif" } else { "png" }
                    ),
                )
                .await?,
            ),
            _ => reply,
        };

        let author = CreateEmbedAuthor::new(&message.author.name).icon_url(
            message
                .author
                .avatar_url()
                .as_deref()
                .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png"),
        );

        let embed = CreateEmbed::new()
            .description(format!(
                "{}\n{}{}",
                message.content,
                message.link(),
                message
                    .channel(ctx)
                    .await
                    .map(|channel| {
                        if let Channel::Guild(channel) = channel {
                            format!(" ({})", channel.name)
                        } else {
                            "".to_string()
                        }
                    })
                    .unwrap_or("".to_string())
            ))
            .author(author)
            .timestamp(message.timestamp);

        let embed = if let Some(attachment) = message.attachments.iter().find(|attachment| {
            attachment
                .content_type
                .as_ref()
                .is_some_and(|content_type| content_type.starts_with("image"))
        }) {
            embed.image(&attachment.url)
        } else {
            embed
        };

        let reply = reply.embed(embed);

        ChannelId::new(self.channel_id)
            .send_message(ctx, reply)
            .await?;

        Ok(())
    }
}

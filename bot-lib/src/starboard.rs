use ahash::AHashSet;
use color_eyre::eyre::Result;
use poise::serenity_prelude::{
    Channel, ChannelId, Context, CreateAttachment, CreateEmbed, CreateEmbedAuthor, CreateMessage,
    GetMessages, Message, MessageId, Reaction, ReactionType,
};
use serde::Deserialize;
use tokio::sync::Mutex;

#[derive(Debug, Deserialize)]
pub struct Starboard {
    pub reaction_count: u64,
    /// Currently only supports unicode emojis.
    pub banned_reactions: Option<Vec<String>>,
    pub channel_id: u64,
    pub ignored_channel_ids: Option<Vec<u64>>,
    /// This stores a string hash of the message link
    #[serde(skip)]
    pub recently_added_messages: Mutex<AHashSet<MessageId>>,
}

impl Starboard {
    #[tracing::instrument(level = "trace", skip(self, ctx, message), fields(message_link = %message.link()))]
    pub async fn does_starboard_apply(
        &self,
        ctx: &Context,
        message: &Message,
        reaction: &Reaction,
    ) -> bool {
        self.is_allowed_reaction(reaction)
            && self.is_channel_allowed(message.channel_id.into())
            && self.enough_reactions(message, reaction)
            && self.is_channel_missing_reply(ctx, message).await
    }

    fn enough_reactions(&self, message: &Message, reaction: &Reaction) -> bool {
        let reaction_type = &reaction.emoji;
        let reaction_count = message
            .reactions
            .iter()
            .find(|reaction| reaction.reaction_type == *reaction_type)
            .map_or(0, |reaction| reaction.count);

        reaction_count >= self.reaction_count
    }

    fn is_allowed_reaction(&self, reaction: &Reaction) -> bool {
        if !matches!(reaction.emoji, ReactionType::Unicode(_)) {
            return true;
        }

        self.banned_reactions
            .as_ref()
            .map_or(true, |banned_reactions| {
                !banned_reactions
                    .iter()
                    .any(|banned_reaction| reaction.emoji.unicode_eq(banned_reaction))
            })
    }

    fn is_channel_allowed(&self, channel_id: u64) -> bool {
        if let Some(ignored_channel_ids) = self.ignored_channel_ids.as_ref() {
            !ignored_channel_ids.contains(&channel_id)
        } else {
            true
        }
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

    pub async fn reply(&self, ctx: &Context, message: &Message, reaction: &Reaction) -> Result<()> {
        let reply = CreateMessage::new();

        let reply = match &reaction.emoji {
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

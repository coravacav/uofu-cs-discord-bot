use color_eyre::eyre::Result;
use poise::serenity_prelude::{
    ChannelId, Context, CreateAllowedMentions, CreateAttachment, CreateMessage, GetMessages, Mentionable, Message, MessageId, MessageReference, MessageReferenceKind, Reaction, ReactionType
};
use rustc_hash::FxHashSet;
use serde::Deserialize;
use tokio::sync::{Mutex, MutexGuard};
use crate::{commands::is_stefan, data::PoiseContext};

#[derive(Deserialize)]
pub struct Starboard {
    pub reaction_count: u64,
    /// Currently only supports unicode emojis.
    pub banned_reactions: Option<Vec<String>>,
    pub channel_id: u64,
    pub ignored_channel_ids: Option<Vec<u64>>,
    /// This stores a string hash of the message link
    #[serde(skip)]
    pub recently_added_messages: Mutex<FxHashSet<MessageId>>,
    #[serde(skip)]
    sequential_message_lock: Mutex<()>,
}

impl Starboard {
    #[tracing::instrument(level = "trace", skip(self, ctx, message), fields(message_link = %message.link()))]
    /// `recent_messages` is used to prevent all starboarding when a single banned reaction is used
    pub async fn does_starboard_apply(
        &self,
        ctx: &Context,
        message: &Message,
        reaction: &Reaction,
        recent_messages: &mut MutexGuard<'_, FxHashSet<MessageId>>,
    ) -> bool {
        self.enough_reactions(message, reaction)
            && self.is_allowed_reaction(reaction, message, recent_messages)
            && self.is_channel_allowed(message.channel_id.into())
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

    fn is_allowed_reaction(
        &self,
        reaction: &Reaction,
        message: &Message,
        recent_messages: &mut MutexGuard<'_, FxHashSet<MessageId>>,
    ) -> bool {
        if !matches!(reaction.emoji, ReactionType::Unicode(_)) {
            return true;
        }

        let banned = !self
            .banned_reactions
            .as_ref()
            .is_some_and(|banned_reactions| {
                banned_reactions
                    .iter()
                    .any(|banned_reaction| reaction.emoji.unicode_eq(banned_reaction))
            });

        // Prevent future reactions from starboarding.
        if banned {
            recent_messages.insert(message.id);
        }

        banned
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

    pub(crate) async fn reply(&self, ctx: &Context, message: &Message, reaction: &ReactionType) -> Result<()> {
        // Ensure that these two messages are back to back
        let _ = self.sequential_message_lock.lock().await;
        
        let _ = ChannelId::new(self.channel_id)
            .send_message(ctx, CreateMessage::new().content(format!(
            "{message_author} in <#{channel_id}> ({channel_name})",
                message_author = message.author.mention(),
                channel_id = message.channel_id,
                channel_name = message.channel_id.name(ctx).await.unwrap_or("unknown".into()),
            )).allowed_mentions(CreateAllowedMentions::new()))
            .await;

        ChannelId::new(self.channel_id)
            .send_message(ctx, CreateMessage::new().reference_message(MessageReference::new(MessageReferenceKind::Forward, message.channel_id)
                .message_id(message.id)))
            .await?;

        let emoji_message = CreateMessage::new();
        let mut send_emoji_message = true;
        let emoji_message = match &reaction {
            ReactionType::Unicode(emoji) => emoji_message.content(emoji),
            ReactionType::Custom { animated, id, .. } => emoji_message.add_file(
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
            _ => {
                send_emoji_message = false;
                emoji_message
            },
        };

        if send_emoji_message {
            ChannelId::new(self.channel_id)
                .send_message(ctx, emoji_message)
                .await?;
        }


        Ok(())
    }
}

#[poise::command(
    prefix_command, 
    check = is_stefan
)]
pub async fn debug_force_starboard(ctx: PoiseContext<'_>, message: Message) -> Result<()> {
    let emoji = ReactionType::Unicode("🧪".into());
    let config = ctx.data().config.read().await;
    for starboard in &config.starboards {
        starboard.reply(ctx.serenity_context(), &message, &emoji).await?;
    }

    Ok(())
}

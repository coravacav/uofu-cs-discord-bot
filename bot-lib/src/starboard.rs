use crate::{
    commands::is_stefan,
    data::{DB, PoiseContext},
    utils::SendReplyEphemeral,
};
use color_eyre::eyre::Result;
use poise::serenity_prelude::{
    ChannelId, Context, CreateAllowedMentions, CreateAttachment, CreateMessage, Mentionable,
    Message, MessageId, MessageReference, MessageReferenceKind, Reaction, ReactionType,
};
use serde::Deserialize;
use surrealdb::RecordId;
use tokio::sync::Mutex;

#[derive(Deserialize)]
pub struct Starboard {
    pub reaction_count: u64,
    /// Currently only supports unicode emojis.
    pub banned_reactions: Option<Vec<String>>,
    pub channel_id: u64,
    pub ignored_channel_ids: Option<Vec<u64>>,
    #[serde(skip)]
    sequential_message_lock: Mutex<()>,
}

impl Starboard {
    #[tracing::instrument(level = "trace", skip(self, message), fields(message_link = %message.link()))]
    /// `recent_messages` is used to prevent all starboarding when a single banned reaction is used
    pub fn does_starboard_apply(&self, message: &Message, reaction: &Reaction) -> bool {
        self.enough_reactions(message, reaction)
            && self.is_allowed_reaction(reaction, message)
            && self.is_channel_allowed(message.channel_id.into())
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

    fn is_allowed_reaction(&self, reaction: &Reaction, message: &Message) -> bool {
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
            tokio::spawn({
                let message_id = message.id;
                async move { Self::insert_recent_message(message_id).await }
            });
        }

        banned
    }

    pub async fn insert_recent_message(message_id: MessageId) -> Result<()> {
        let message_id = i64::from(message_id);
        let _ = DB
            .query("create $message")
            .bind((
                "message",
                RecordId::from(("starboard_recent_message", message_id)),
            ))
            .await?;
        Ok(())
    }

    pub async fn has_recent_message(message_id: MessageId) -> Result<bool> {
        let message_id = i64::from(message_id);
        Ok(DB
            .query("$message.exists();")
            .bind((
                "message",
                RecordId::from(("starboard_recent_message", message_id)),
            ))
            .await?
            .take::<Option<bool>>(0)?
            .unwrap_or(false))
    }

    pub async fn ignore_message_permanently(message_id: MessageId) -> Result<()> {
        let message_id = i64::from(message_id);
        let _ = DB
            .query("create $message")
            .bind((
                "message",
                RecordId::from(("starboard_recent_message", message_id)),
            ))
            .await?;
        Ok(())
    }

    fn is_channel_allowed(&self, channel_id: u64) -> bool {
        if let Some(ignored_channel_ids) = self.ignored_channel_ids.as_ref() {
            !ignored_channel_ids.contains(&channel_id)
        } else {
            true
        }
    }

    pub(crate) async fn reply(
        &self,
        ctx: &Context,
        message: &Message,
        reaction: &ReactionType,
    ) -> Result<()> {
        // Ensure that these two messages are back to back
        let _lock = self.sequential_message_lock.lock().await;

        let _ = ChannelId::new(self.channel_id)
            .send_message(
                ctx,
                CreateMessage::new()
                    .content(format!(
                        "{message_author} in <#{channel_id}> ({channel_name})",
                        message_author = message.author.mention(),
                        channel_id = message.channel_id,
                        channel_name = message
                            .channel_id
                            .name(ctx)
                            .await
                            .unwrap_or("unknown".into()),
                    ))
                    .allowed_mentions(CreateAllowedMentions::new()),
            )
            .await;

        ChannelId::new(self.channel_id)
            .send_message(
                ctx,
                CreateMessage::new().reference_message(
                    MessageReference::new(MessageReferenceKind::Forward, message.channel_id)
                        .message_id(message.id),
                ),
            )
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
            }
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
        starboard
            .reply(ctx.serenity_context(), &message, &emoji)
            .await?;
    }

    Ok(())
}

#[poise::command(
    prefix_command,
    check = is_stefan
)]
pub async fn debug_surrealdb(ctx: PoiseContext<'_>, query: Vec<String>) -> Result<()> {
    let reply = DB.query(query.join(" ")).await?;
    ctx.reply_ephemeral(format!("{:?}", reply)).await?;

    Ok(())
}

#[cfg(test)]
#[tokio::test]
async fn test_db_setup() {
    use poise::serenity_prelude::MessageId;

    use crate::{data::setup_db, starboard::Starboard};

    setup_db().await;
    assert!(DB.health().await.is_ok());

    Starboard::insert_recent_message(MessageId::from(1))
        .await
        .unwrap();

    assert!(
        Starboard::has_recent_message(MessageId::from(1))
            .await
            .unwrap()
    );

    assert!(
        !Starboard::has_recent_message(MessageId::from(2))
            .await
            .unwrap()
    );
}

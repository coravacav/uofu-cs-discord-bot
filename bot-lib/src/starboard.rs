use crate::{
    commands::is_stefan,
    data::{PoiseContext, with_db},
    utils::SendReplyEphemeral,
};
use color_eyre::eyre::Result;
use poise::serenity_prelude::{
    ChannelId, Context, CreateAllowedMentions, CreateAttachment, CreateMessage, Mentionable,
    Message, MessageId, MessageReference, MessageReferenceKind, Reaction, ReactionType,
};
use serde::Deserialize;
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
    /// Checks the reaction threshold and configured channel/reaction exclusions.
    pub fn does_starboard_apply(&self, message: &Message, reaction: &Reaction) -> bool {
        self.enough_reactions(message, reaction)
            && self.is_allowed_reaction(reaction)
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

    fn is_allowed_reaction(&self, reaction: &Reaction) -> bool {
        if !matches!(reaction.emoji, ReactionType::Unicode(_)) {
            return true;
        }

        !self
            .banned_reactions
            .as_ref()
            .is_some_and(|banned_reactions| {
                banned_reactions
                    .iter()
                    .any(|banned_reaction| reaction.emoji.unicode_eq(banned_reaction))
            })
    }

    /// Claims a message for the starboard. Fails if it was already claimed.
    pub async fn insert_recent_message(message_id: MessageId) -> Result<()> {
        let message_id = u64::from(message_id);
        with_db(move |conn| {
            conn.execute(
                "INSERT INTO starboard_recent_message (message_id) VALUES (?1)",
                [message_id],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn has_recent_message(message_id: MessageId) -> Result<bool> {
        let message_id = u64::from(message_id);
        with_db(move |conn| {
            Ok(conn.query_row(
                "SELECT EXISTS (SELECT 1 FROM starboard_recent_message WHERE message_id = ?1)",
                [message_id],
                |row| row.get(0),
            )?)
        })
        .await
    }

    pub async fn ignore_message_permanently(message_id: MessageId) -> Result<()> {
        Self::insert_recent_message(message_id).await
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
pub async fn debug_sql(ctx: PoiseContext<'_>, query: Vec<String>) -> Result<()> {
    let sql = query.join(" ");
    let reply = with_db(move |conn| {
        let mut statement = conn.prepare(&sql)?;
        let column_count = statement.column_count();
        if column_count == 0 {
            return Ok(format!("{} rows changed", statement.execute([])?));
        }

        let columns = statement.column_names().join(" | ");
        let rows = statement
            .query_map([], |row| {
                (0..column_count)
                    .map(|i| row.get::<_, rusqlite::types::Value>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })?
            .map(|row| Ok(format!("{:?}", row?)))
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(format!("{columns}\n{}", rows.join("\n")))
    })
    .await?;

    // Discord rejects messages over 2000 characters.
    let reply: String = reply.chars().take(1990).collect();
    ctx.reply_ephemeral(format!("```\n{reply}\n```")).await?;

    Ok(())
}

#[cfg(test)]
#[tokio::test]
async fn test_db_setup() {
    use poise::serenity_prelude::MessageId;

    use crate::{data::setup_db, starboard::Starboard};

    setup_db();

    Starboard::insert_recent_message(MessageId::from(1))
        .await
        .unwrap();

    assert!(
        Starboard::insert_recent_message(MessageId::from(1))
            .await
            .is_err(),
        "a duplicate marker must surface its statement error"
    );

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

    crate::economy::tests::assert_economy_is_persisted_and_ranked().await;
}

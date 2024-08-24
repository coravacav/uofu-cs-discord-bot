use color_eyre::eyre::Result;
use parking_lot::RwLock;
use poise::serenity_prelude::ChannelId;
use poise::serenity_prelude::{self as serenity};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EmoteType {
    AllEmotes { all_emotes: bool },
    CustomEmote { emote_name: String },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Starboard {
    pub reaction_count: u64,
    pub channel_id: u64,
    pub ignored_channel_ids: Option<Vec<u64>>,
    #[serde(flatten)]
    pub emote_type: EmoteType,
    /// This stores a string hash of the message link
    #[serde(skip)]
    pub recently_added_messages: RwLock<HashSet<String>>,
}

impl PartialEq for Starboard {
    fn eq(&self, other: &Self) -> bool {
        self.reaction_count == other.reaction_count
            && self.channel_id == other.channel_id
            && self.ignored_channel_ids == other.ignored_channel_ids
            && self.emote_type == other.emote_type
    }
}

impl Eq for Starboard {}

impl Default for Starboard {
    fn default() -> Self {
        Self {
            reaction_count: 1,
            channel_id: 0,
            ignored_channel_ids: None,
            emote_type: EmoteType::AllEmotes { all_emotes: true },
            recently_added_messages: RwLock::new(HashSet::new()),
        }
    }
}

impl Starboard {
    #[tracing::instrument(level = "trace", skip(self, ctx, message), fields(message_link = %message.link()))]
    pub async fn does_starboard_apply(
        &self,
        ctx: &serenity::Context,
        message: &serenity::Message,
        reaction_count: u64,
        emote_name: &str,
    ) -> bool {
        let check = self.enough_reactions(reaction_count)
            && self.is_message_recent(&message.timestamp)
            && self.is_channel_allowed(message.channel_id.into())
            && self.is_emote_allowed(emote_name)
            && self.is_message_unseen(&message.link())
            && self.is_message_a_lynch(message).await
            && self.is_channel_missing_reply(ctx, message).await;

        let check_msg = if check { "applies" } else { "does not apply" };
        tracing::trace!("starboard {}", check_msg);

        check
    }

    fn enough_reactions(&self, reaction_count: u64) -> bool {
        let check = reaction_count >= self.reaction_count;
        let check_text = if check { "enough" } else { "not enough" };

        tracing::trace!(
            "reaction_count {} is {} (needed {})",
            reaction_count,
            check_text,
            self.reaction_count
        );

        check
    }

    const ONE_WEEK: chrono::TimeDelta = match chrono::TimeDelta::try_weeks(1) {
        Some(time_check) => time_check,
        None => panic!("Failed to create time check"),
    };

    fn is_message_recent(&self, message_timestamp: &serenity::Timestamp) -> bool {
        let message_timestamp = message_timestamp.unix_timestamp();
        let check = message_timestamp > (chrono::Utc::now() - Self::ONE_WEEK).timestamp();

        let check_text = if check { "new enough" } else { "too old" };

        tracing::trace!("message is {}", check_text);

        check
    }

    fn is_channel_allowed(&self, channel_id: u64) -> bool {
        let check = self
            .ignored_channel_ids
            .as_ref()
            .map(|ignored_channel_ids| !ignored_channel_ids.contains(&channel_id))
            .unwrap_or(true);

        let check_text = if check { "allowed" } else { "disallowed" };
        tracing::trace!("channel_id {} is {}", channel_id, check_text);

        check
    }

    fn is_emote_allowed(&self, emote_name: &str) -> bool {
        let check = match &self.emote_type {
            EmoteType::AllEmotes { .. } => true,
            EmoteType::CustomEmote {
                emote_name: allowed_emote_name,
            } => emote_name == allowed_emote_name,
        };

        let check_text = if check { "allowed" } else { "disallowed" };
        tracing::trace!("emote_name {} is {}", emote_name, check_text);

        check
    }

    fn is_message_unseen(&self, message_link: &str) -> bool {
        let check = !self.recently_added_messages.read().contains(message_link);

        let check_text = if check { "seen" } else { "unseen" };
        tracing::trace!("message_link {} is {}", message_link, check_text);

        check
    }

    async fn is_message_a_lynch(&self, message: &serenity::Message) -> bool {
        use crate::commands::lynch::{LYNCH_KNOWN_MESSAGE_PORTION, LYNCH_MAP};

        !LYNCH_MAP.contains_key(&message.id)
            && !message.content.starts_with(LYNCH_KNOWN_MESSAGE_PORTION)
    }

    async fn is_channel_missing_reply(
        &self,
        ctx: &serenity::Context,
        message: &serenity::Message,
    ) -> bool {
        let message_link = message.link();

        let Ok(messages) = ChannelId::new(self.channel_id)
            .messages(ctx, serenity::GetMessages::new())
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

        let check = !has_already_been_added;

        let check_text = if check { "missing" } else { "already added" };
        tracing::trace!("message_link {} is {}", message_link, check_text);

        check
    }

    pub async fn reply(
        &self,
        ctx: &serenity::Context,
        message: &serenity::Message,
        reaction_type: &serenity::ReactionType,
    ) -> Result<()> {
        let reply = serenity::CreateMessage::new();

        let reply = match reaction_type {
            serenity::ReactionType::Unicode(emoji) => reply.content(emoji),
            serenity::ReactionType::Custom { animated, id, .. } => reply.add_file(
                serenity::CreateAttachment::url(
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

        let author = serenity::CreateEmbedAuthor::new(&message.author.name).icon_url(
            message
                .author
                .avatar_url()
                .as_deref()
                .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png"),
        );

        let embed = serenity::CreateEmbed::new()
            .description(format!(
                "{}\n{}{}",
                message.content,
                message.link(),
                message
                    .channel(ctx)
                    .await
                    .map(|channel| {
                        if let serenity::Channel::Guild(channel) = channel {
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

        self.recently_added_messages.write().insert(message.link());

        Ok(())
    }
}

#[test]
fn check_deserialize_emote() {
    let toml = r#"
reaction_count = 1
channel_id = 1
emote_name = "test"
"#;
    let starboard: Starboard = toml::from_str(toml).unwrap();

    assert_eq!(
        starboard,
        Starboard {
            reaction_count: 1,
            channel_id: 1,
            emote_type: EmoteType::CustomEmote {
                emote_name: "test".to_string()
            },
            ..Default::default()
        }
    );
}

#[test]
fn check_deserialize_all_emotes() {
    let toml = r#"
reaction_count = 1
channel_id = 1
all_emotes = true
"#;
    let starboard: Starboard = toml::from_str(toml).unwrap();

    assert_eq!(
        starboard,
        Starboard {
            reaction_count: 1,
            channel_id: 1,
            emote_type: EmoteType::AllEmotes { all_emotes: true },
            ..Default::default()
        }
    );
}

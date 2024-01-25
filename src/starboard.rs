use poise::serenity_prelude::ChannelId;
use poise::serenity_prelude::{self as serenity};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

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
    pub recently_added_messages: Arc<RwLock<HashSet<String>>>,
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
            recently_added_messages: Arc::new(RwLock::new(HashSet::new())),
        }
    }
}

impl Starboard {
    pub async fn does_starboard_apply(
        &self,
        reaction_count: u64,
        emote_name: &str,
        channel_id: u64,
    ) -> bool {
        if let Some(ignored_channel_ids) = &self.ignored_channel_ids {
            if ignored_channel_ids.contains(&channel_id) {
                return false;
            }
        }

        if let EmoteType::CustomEmote {
            emote_name: specific_emote,
        } = &self.emote_type
        {
            if specific_emote != emote_name {
                return false;
            }
        }

        if reaction_count < self.reaction_count {
            return false;
        }

        true
    }

    pub async fn does_channel_already_have_reply(
        &self,
        ctx: &serenity::Context,
        message: &serenity::Message,
    ) -> anyhow::Result<bool> {
        let message_link = message.link();

        if self
            .recently_added_messages
            .read()
            .map(|recent_messages| recent_messages.contains(&message_link))
            .unwrap_or(false)
        {
            return Ok(true);
        }

        let recent_messages = ChannelId::new(self.channel_id)
            .messages(ctx, serenity::GetMessages::new())
            .await?;

        let has_already_been_added = recent_messages.iter().any(|message| {
            message.embeds.iter().any(|embed| {
                embed
                    .description
                    .as_ref()
                    .is_some_and(|description| description.contains(&message_link))
            })
        });

        Ok(has_already_been_added)
    }

    pub async fn generate_reply(
        &self,
        ctx: &serenity::Context,
        message: &serenity::Message,
        reaction_type: &serenity::ReactionType,
    ) -> anyhow::Result<()> {
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

        self.recently_added_messages
            .write()
            .map(|mut recent_messages| {
                recent_messages.insert(message.link());
            })
            .map_err(|_| {
                anyhow::anyhow!("Failed to insert message link into recently added messages")
            })
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

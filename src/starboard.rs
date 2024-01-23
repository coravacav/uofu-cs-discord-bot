use poise::serenity_prelude::ChannelId;
use poise::serenity_prelude::{self as serenity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(untagged)]
pub enum EmoteType {
    AllEmotes { all_emotes: bool },
    CustomEmote { emote_name: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Starboard {
    pub reaction_count: u64,
    pub channel_id: u64,
    pub ignored_channels: Option<Vec<u64>>,
    #[serde(default)]
    pub ignore_posts: bool,
    #[serde(flatten)]
    pub emote_type: EmoteType,
}

impl Default for Starboard {
    fn default() -> Self {
        Self {
            reaction_count: 1,
            channel_id: 0,
            ignored_channels: None,
            ignore_posts: true,
            emote_type: EmoteType::AllEmotes { all_emotes: true },
        }
    }
}

impl Starboard {
    pub async fn does_starboard_apply(&self, reaction_count: u64, emote_name: &str) -> bool {
        (match &self.emote_type {
            EmoteType::AllEmotes { .. } => true,
            EmoteType::CustomEmote {
                emote_name: starboard_emote_name,
            } => emote_name == starboard_emote_name,
        }) && reaction_count >= self.reaction_count
    }

    pub async fn does_channel_already_have_reply(
        &self,
        ctx: &serenity::Context,
        message_link: &str,
    ) -> anyhow::Result<bool> {
        let recent_messages = ChannelId::new(self.channel_id)
            .messages(ctx, serenity::GetMessages::new())
            .await?;

        let has_already_been_added = recent_messages.iter().any(|message| {
            message.embeds.iter().any(|embed| {
                embed
                    .description
                    .as_ref()
                    .is_some_and(|description| description.contains(message_link))
            })
        });

        Ok(has_already_been_added)
    }

    pub async fn generate_reply<'a, 'b>(
        &'a self,
        ctx: &'a serenity::Context,
        message: &'a serenity::Message,
        reaction_type: &'a serenity::ReactionType,
    ) -> anyhow::Result<()> {
        let reply = serenity::CreateMessage::new();

        let reply = match reaction_type {
            serenity::ReactionType::Unicode(emoji) => reply.content(format!("{}", emoji)),
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
            .description(format!("{}\n{}", message.content, message.link()))
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

        ChannelId::new(self.channel_id).send_message(ctx, reply);

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

use poise::serenity_prelude as serenity;
use poise::serenity_prelude::ChannelId;
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
    #[serde(flatten)]
    pub emote_type: EmoteType,
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
        let channel_id = ChannelId(self.channel_id);
        let recent_messages = channel_id.messages(ctx, |m| m).await?;

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
        ChannelId(self.channel_id)
            .send_message(ctx, |new_message| {
                match reaction_type {
                    serenity::ReactionType::Unicode(emoji) => {
                        new_message.content(format!("{}", emoji));
                    }
                    serenity::ReactionType::Custom { animated, id, .. } => {
                        if let Ok(url) = format!(
                            "https://cdn.discordapp.com/emojis/{}.{}",
                            id,
                            if *animated { "gif" } else { "png" }
                        )
                        .parse()
                        {
                            new_message.add_file(serenity::AttachmentType::Image(url));
                        }
                    }
                    _ => (),
                };

                new_message.add_embed(|embed| {
                    embed
                        .description(format!("{}\n{}", message.content, message.link()))
                        .author(|author| {
                            author.name(&message.author.name).icon_url(
                                message
                                    .author
                                    .avatar_url()
                                    .as_deref()
                                    .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png"),
                            )
                        })
                        .timestamp(message.timestamp);

                    if let Some(attachment) = message.attachments.iter().find(|attachment| {
                        attachment
                            .content_type
                            .as_ref()
                            .is_some_and(|content_type| content_type.starts_with("image"))
                    }) {
                        embed.image(&attachment.url)
                    } else {
                        embed
                    }
                })
            })
            .await?;

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
            }
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
            emote_type: EmoteType::AllEmotes { all_emotes: true }
        }
    );
}

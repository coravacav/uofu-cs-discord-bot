use crate::data::PoiseContext;
use color_eyre::eyre::Result;
use poise::serenity_prelude::{ChannelId, CreateEmbed, CreateEmbedAuthor, CreateMessage};

const ANON_CHANNEL_ID: ChannelId = ChannelId::new(1274560000102236282);

/// Without posting to chat, send the mods a message.
///
/// Useful for someone being annoying even if they don't break the rules (or they do).
///
/// Serious uses only.
#[poise::command(slash_command, ephemeral = true)]
pub async fn anon_notify(ctx: PoiseContext<'_>, message: String) -> Result<()> {
    let author = ctx.author();
    let author = CreateEmbedAuthor::new(&author.name).icon_url(
        author
            .avatar_url()
            .as_deref()
            .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png"),
    );

    ANON_CHANNEL_ID
        .send_message(
            ctx,
            CreateMessage::new().embed(
                CreateEmbed::new()
                    .title("Used `/anon_notify`. Do not let it be abused.")
                    .author(author)
                    .description(message),
            ),
        )
        .await?;

    ctx.say("Sent your message").await?;

    Ok(())
}

use color_eyre::eyre::Result;
use poise::serenity_prelude::EditMessage;

use crate::{
    data::PoiseContext,
    text_detection::{KINGFISHER_REPLY_LAST_BY_USER, KingfisherReplyMetadata},
};

/// Use this command to change the reply that kingfisher made to you last.
///
/// If there are no other options, it will still try, but never change.
#[poise::command(slash_command, ephemeral = true)]
pub async fn reroll_reply(ctx: PoiseContext<'_>) -> Result<()> {
    let author = ctx.author();

    let Some(KingfisherReplyMetadata {
        message_id,
        channel_id,
        response,
    }) = KINGFISHER_REPLY_LAST_BY_USER
        .lock()
        .get(&author.id)
        .cloned()
    else {
        ctx.say("No reply to reroll").await?;

        return Ok(());
    };

    let Some(response_text) = response.get_reply_text() else {
        ctx.say("No response found for message. Weird. Let Stefan know")
            .await?;

        return Ok(());
    };

    channel_id
        .edit_message(
            &ctx,
            message_id,
            EditMessage::new().content(response_text.as_ref()),
        )
        .await?;

    ctx.say("Rerolled reply!").await?;

    Ok(())
}

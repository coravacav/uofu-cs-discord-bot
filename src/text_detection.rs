use crate::data::Data;
use poise::serenity_prelude as serenity;
use serenity::Message;

pub async fn text_detection(
    ctx: &serenity::Context,
    data: &Data,
    message: &Message,
) -> anyhow::Result<()> {
    if message.is_own(ctx) {
        return Ok(());
    }

    if let Some(message_response) = data.find_response(&message.content).await {
        data.run_action(&message_response, message, ctx).await?;
    }

    Ok(())
}

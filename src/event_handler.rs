use poise::serenity_prelude as serenity;

use crate::{data::Data, reaction_management::reaction_management, text_detection::text_detection};

pub async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    framework: poise::FrameworkContext<'_, Data, anyhow::Error>,
    _data: &Data,
) -> anyhow::Result<()> {
    match event {
        serenity::FullEvent::Message { new_message } => {
            text_detection(ctx, framework.user_data, new_message).await
        }
        serenity::FullEvent::ReactionAdd { add_reaction } => {
            reaction_management(ctx, framework.user_data, add_reaction).await
        }
        serenity::FullEvent::ReactionRemove { removed_reaction } => {
            reaction_management(ctx, framework.user_data, removed_reaction).await
        }
        _ => Ok(()),
    }
}

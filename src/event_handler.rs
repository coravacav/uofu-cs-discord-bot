use poise::{serenity_prelude as serenity, Event};

use crate::{data::Data, reaction_management::reaction_management, text_detection::text_detection};

pub async fn event_handler(
    ctx: &serenity::Context,
    event: &Event<'_>,
    framework: poise::FrameworkContext<'_, Data, anyhow::Error>,
    _data: &Data,
) -> anyhow::Result<()> {
    match event {
        Event::Message { new_message } => {
            text_detection(ctx, framework.user_data, new_message).await
        }
        Event::ReactionAdd { add_reaction } => {
            reaction_management(ctx, framework.user_data, add_reaction).await
        }
        Event::ReactionRemove { removed_reaction } => {
            reaction_management(ctx, framework.user_data, removed_reaction).await
        }
        _ => Ok(()),
    }
}

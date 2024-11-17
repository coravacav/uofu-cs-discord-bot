use crate::data::State;
use color_eyre::eyre::Result;
use poise::serenity_prelude::{Context, Message, Reaction, Timestamp};

pub async fn handle_starboards(
    ctx: &Context,
    data: State,
    message: &Message,
    reaction: &Reaction,
) -> Result<()> {
    if is_message_too_recent(&message.timestamp) || is_message_yeet(message) {
        return Ok(());
    }

    let reaction_type = &reaction.emoji;

    let reaction_count = message
        .reactions
        .iter()
        .find(|reaction| reaction.reaction_type == *reaction_type)
        .map_or(0, |reaction| reaction.count);

    let config = data.config.read().await;

    let futures = config.starboards.iter().map(|starboard| async {
        if starboard
            .does_starboard_apply(ctx, message, reaction_count)
            .await
        {
            starboard.reply(ctx, message, reaction_type).await.ok();
        }
    });

    futures::future::join_all(futures).await;

    Ok(())
}

fn is_message_too_recent(message_timestamp: &Timestamp) -> bool {
    const ONE_WEEK: chrono::TimeDelta = match chrono::TimeDelta::try_weeks(1) {
        Some(time_check) => time_check,
        None => unreachable!(),
    };

    message_timestamp.unix_timestamp() > (chrono::Utc::now() - ONE_WEEK).timestamp()
}

fn is_message_yeet(message: &Message) -> bool {
    !crate::commands::YEET_STARBOARD_EXCLUSIONS
        .lock()
        .contains(&message.id)
}

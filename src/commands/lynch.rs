use std::{collections::HashMap, time::Duration};

use crate::{data::PoiseContext, utils::GetRelativeTimestamp};
use color_eyre::eyre::{eyre, OptionExt, Result, WrapErr};
use lazy_static::lazy_static;
use poise::serenity_prelude::{
    self as serenity, ChannelId, CreateMessage, GuildId, MessageBuilder, MessageId, User,
};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct LynchData {
    lyncher: User,
    victim: User,
    guild_id: GuildId,
    channel_id: ChannelId,
}

pub const LYNCH_DEFAULT_OPPORTUNITIES: usize = 3;
pub const LYNCH_REQUIRED_REACTION_COUNT: u64 = 6;
pub const LYNCH_NO_REACTION: char = '❌';
pub const LYNCH_YES_REACTION: char = '✅';
pub const LYNCH_DURATION_SECONDS: u64 = 300;
pub const LYNCH_VOTING_SECONDS: u64 = 90;
pub const LYNCH_KNOWN_MESSAGE_PORTION: &str = "Do you want to lynch ";

lazy_static! {
    pub static ref LYNCH_MAP: Mutex<HashMap<MessageId, LynchData>> = Mutex::new(HashMap::new());
    pub static ref LYNCH_OPPORTUNITIES: Mutex<usize> = Mutex::new(LYNCH_DEFAULT_OPPORTUNITIES);
}

#[poise::command(
    slash_command,
    prefix_command,
    rename = "lynch",
    ephemeral = true,
    description_localized(
        "en-US",
        "Lynch a user if you get 6 yay votes, get lynched yourself if they vote nay"
    )
)]
pub async fn lynch(ctx: PoiseContext<'_>, victim: User) -> Result<()> {
    tracing::trace!("lynch start");

    let lyncher = ctx.author().clone();
    let guild_id = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let channel_id = ctx.channel_id();
    let react_role_id = ctx.data().config.read().await.bot_react_role_id;

    if !victim.has_role(ctx, guild_id, react_role_id).await? {
        ctx.say("You can't lynch a non reactme user!").await?;
        return Ok(());
    }

    let mut lynch_opportunities = LYNCH_OPPORTUNITIES.lock().await;

    if *lynch_opportunities == 0 {
        ctx.say("No more lynch opportunities available").await?;
        tracing::info!("No more lynch opportunities available");
        return Ok(());
    }

    *lynch_opportunities = (*lynch_opportunities).saturating_sub(1);
    tracing::info!("Updated lynch opportunities to {lynch_opportunities}");
    tracing::info!("lynched {}", victim.name);
    drop(lynch_opportunities);

    let msg = CreateMessage::new()
        .content(
            MessageBuilder::new()
                .push(LYNCH_KNOWN_MESSAGE_PORTION)
                .mention(&victim)
                .push(format!(
                    "? ({} {}'s needed)\n",
                    LYNCH_REQUIRED_REACTION_COUNT, LYNCH_YES_REACTION,
                ))
                .push(format!(
                    "Or, vote {} to lynch the author: ||",
                    LYNCH_NO_REACTION
                ))
                .mention(&lyncher)
                .push("||\n")
                .push("Otherwise, this will be deleted ")
                .push(
                    (chrono::Utc::now() + Duration::from_secs(LYNCH_VOTING_SECONDS))
                        .discord_relative_timestamp(),
                )
                .build(),
        )
        .reactions([LYNCH_YES_REACTION, LYNCH_NO_REACTION]);

    let Ok(msg) = channel_id.send_message(ctx, msg).await else {
        ctx.say("Couldn't send message announcing lynching").await?;
        return Err(eyre!("Couldn't send message announcing lynching"));
    };

    ctx.say("Lynching started!").await?;

    LYNCH_MAP.lock().await.insert(
        msg.id,
        LynchData {
            lyncher,
            victim,
            guild_id,
            channel_id,
        },
    );

    tokio::time::sleep(Duration::from_secs(LYNCH_VOTING_SECONDS)).await;

    if LYNCH_MAP.lock().await.remove(&msg.id).is_some() {
        msg.delete(ctx).await?;
    }

    Ok(())
}

pub async fn update_interval() {
    use futures::StreamExt;

    // Every 1 hour, add a lynch opportunity up to the default, tokio interval
    tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(Duration::from_secs(3600)))
        .for_each(|_| async {
            let mut lynch_opportunities = LYNCH_OPPORTUNITIES.lock().await;
            *lynch_opportunities = (*lynch_opportunities + 1).min(LYNCH_DEFAULT_OPPORTUNITIES);
            tracing::info!("Updated lynch opportunities to {lynch_opportunities}");
        })
        .await
}

// Handle a reaction
pub async fn handle_lynching(ctx: &serenity::Context, message: &serenity::Message) -> Result<()> {
    // check if message is in the lynch map
    let lynch_data = match LYNCH_MAP.lock().await.get(&message.id) {
        Some(data) => data.clone(),
        None => return Ok(()),
    };

    // count reaction counts on yay and nay
    let mut yay = 0;
    let mut nay = 0;
    for reaction in &message.reactions {
        match &reaction.reaction_type {
            serenity::ReactionType::Unicode(str) => match str.chars().next() {
                Some(LYNCH_NO_REACTION) => nay = reaction.count,
                Some(LYNCH_YES_REACTION) => yay = reaction.count,
                _ => continue,
            },
            _ => continue,
        }
    }

    let yay = yay >= LYNCH_REQUIRED_REACTION_COUNT;
    let nay = nay >= LYNCH_REQUIRED_REACTION_COUNT;

    if !yay && !nay {
        return Ok(());
    }

    // Delete the voting message
    message.delete(ctx).await?;
    LYNCH_MAP.lock().await.remove(&message.id);

    let target = if yay {
        &lynch_data.victim
    } else {
        &lynch_data.lyncher
    };

    let time = std::time::Duration::from_secs(LYNCH_DURATION_SECONDS);
    let timeout_end = chrono::Utc::now() + time;

    lynch_data
        .guild_id
        .edit_member(
            ctx,
            target.clone(),
            serenity::EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
        )
        .await?;

    let message_handle = lynch_data
        .channel_id
        .send_message(
            ctx,
            CreateMessage::new().content(
                MessageBuilder::new()
                    .push("User ")
                    .mention(target)
                    .push(" has been lynched! They will return ")
                    .push(timeout_end.discord_relative_timestamp())
                    .build(),
            ),
        )
        .await?;

    tokio::time::sleep(time - std::time::Duration::from_secs(1)).await;

    message_handle
        .delete(ctx)
        .await
        .wrap_err("Failed to delete message")?;

    Ok(())
}

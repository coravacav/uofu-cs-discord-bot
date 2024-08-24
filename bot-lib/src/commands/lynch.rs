use crate::{data::PoiseContext, utils::GetRelativeTimestamp};
use color_eyre::eyre::{eyre, OptionExt, Result};
use dashmap::DashMap;
use itertools::Itertools;
use poise::serenity_prelude::{
    self as serenity, ChannelId, CreateMessage, EditMessage, GuildId, Mentionable, MessageBuilder,
    MessageId, User, UserId,
};
use std::{sync::LazyLock, time::Duration};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct LynchData {
    lyncher: UserId,
    victim: UserId,
    guild_id: GuildId,
    channel_id: ChannelId,
}

pub const LYNCH_DEFAULT_OPPORTUNITIES: usize = 3;
pub const LYNCH_REQUIRED_REACTION_COUNT: usize = 6;
pub const LYNCH_NO_REACTION: char = '❌';
pub const LYNCH_YES_REACTION: char = '✅';
pub const LYNCH_DURATION_SECONDS: u64 = 300;
pub const LYNCH_VOTING_SECONDS: u64 = 90;
pub const LYNCH_KNOWN_MESSAGE_PORTION: &str = "Do you want to lynch ";

pub static LYNCH_MAP: LazyLock<DashMap<MessageId, LynchData>> = LazyLock::new(DashMap::new);
pub static LYNCH_OPPORTUNITIES: LazyLock<Mutex<usize>> =
    LazyLock::new(|| Mutex::new(LYNCH_DEFAULT_OPPORTUNITIES));

/// Lynch a user if you get 6 yay votes, get lynched yourself if they vote nay
#[poise::command(slash_command, rename = "lynch")]
pub async fn lynch(ctx: PoiseContext<'_>, victim: User) -> Result<()> {
    tracing::trace!("lynch start");

    let lyncher = ctx.author().id;
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

    LYNCH_MAP.insert(
        msg.id,
        LynchData {
            lyncher,
            victim: victim.id,
            guild_id,
            channel_id,
        },
    );

    ctx.say("Lynching started!").await?;

    tokio::time::sleep(Duration::from_secs(LYNCH_VOTING_SECONDS)).await;

    if LYNCH_MAP.remove(&msg.id).is_some() {
        msg.delete(ctx).await.ok();
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
    let message_id = message.id;

    // check if message is in the lynch map
    let lynch_data = match LYNCH_MAP.get(&message_id) {
        Some(data) => data.clone(),
        None => return Ok(()),
    };

    // Kingfisher user id
    let kingfisher_id = ctx.cache.current_user().id;

    // count reaction counts on yay and nay
    let yay = message
        .reaction_users(ctx, LYNCH_YES_REACTION, None, None)
        .await?
        .into_iter()
        .filter(|user| user.id != kingfisher_id)
        .collect_vec();
    let nay = message
        .reaction_users(ctx, LYNCH_NO_REACTION, None, None)
        .await?
        .into_iter()
        .filter(|user| user.id != kingfisher_id)
        .collect_vec();

    let did_yay = yay.len() >= LYNCH_REQUIRED_REACTION_COUNT;
    let did_nay = nay.len() >= LYNCH_REQUIRED_REACTION_COUNT;

    if !did_yay && !did_nay {
        return Ok(());
    }

    LYNCH_MAP.remove(&message_id);

    // Delete the voting message
    message.delete(ctx).await.ok(); // Don't care if it succeeds

    let (target, shooters) = if did_yay {
        (&lynch_data.victim, yay)
    } else {
        (&lynch_data.lyncher, nay)
    };

    let time = std::time::Duration::from_secs(LYNCH_DURATION_SECONDS);
    let timeout_end = chrono::Utc::now() + time;

    lynch_data
        .guild_id
        .edit_member(
            ctx,
            target,
            serenity::EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
        )
        .await?;

    let mut message_handle = lynch_data
        .channel_id
        .send_message(
            ctx,
            CreateMessage::new().content(
                MessageBuilder::new()
                    .push(format!(
                        "User {} has been lynched! They will return {}\nBrought to you by: {}",
                        target.mention(),
                        timeout_end.discord_relative_timestamp(),
                        shooters.mention_all(),
                    ))
                    .build(),
            ),
        )
        .await?;

    tokio::time::sleep(time - std::time::Duration::from_secs(1)).await;

    message_handle
        .edit(
            ctx,
            EditMessage::new().content(format!(
                "User {} was lynched\nBrought to you by: {}",
                target.mention(),
                shooters.mention_all()
            )),
        )
        .await?;

    Ok(())
}

trait MentionableExt {
    fn mention_all(&self) -> String;
}

impl MentionableExt for Vec<User> {
    fn mention_all(&self) -> String {
        self.iter().map(|user| user.mention().to_string()).join(" ")
    }
}

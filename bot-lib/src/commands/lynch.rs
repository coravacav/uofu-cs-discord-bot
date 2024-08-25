use crate::{
    data::{AppState, PoiseContext},
    db::get_lynch_leaderboard,
    utils::GetRelativeTimestamp,
};
use color_eyre::eyre::{bail, OptionExt, Result};
use core::str;
use dashmap::DashMap;
use itertools::Itertools;
use poise::serenity_prelude::{
    self as serenity, ChannelId, CreateMessage, EditMessage, GuildId, Mentionable, MessageBuilder,
    MessageId, User, UserId,
};
use std::{collections::BinaryHeap, sync::LazyLock, time::Duration};
use tokio::{
    sync::Mutex,
    time::{interval, sleep},
};
use tokio_stream::wrappers::IntervalStream;

#[derive(Clone)]
pub struct LynchData {
    lyncher: UserId,
    victim: UserId,
    guild_id: GuildId,
    channel_id: ChannelId,
}

pub const LYNCH_DEFAULT_OPPORTUNITIES: usize = 3;
pub const LYNCH_REQUIRED_REACTION_COUNT: u64 = 6;
pub const LYNCH_NO_REACTION: char = '❌';
pub const LYNCH_YES_REACTION: char = '✅';
pub const LYNCH_DURATION_SECONDS: u64 = 300;
pub const LYNCH_REFRESH_CHARGE_SECONDS: u64 = 3600;
pub const LYNCH_VOTING_SECONDS: u64 = 90;
pub const LYNCH_KNOWN_MESSAGE_PORTION: &str = "Do you want to lynch ";

pub static LYNCH_MAP: LazyLock<DashMap<MessageId, LynchData>> = LazyLock::new(DashMap::new);
pub static LYNCH_OPPORTUNITIES: LazyLock<Mutex<usize>> =
    LazyLock::new(|| Mutex::new(LYNCH_DEFAULT_OPPORTUNITIES));

async fn check_lynch_opportunities() -> Result<bool> {
    let mut lynch_opportunities = LYNCH_OPPORTUNITIES.lock().await;

    if *lynch_opportunities == 0 {
        return Ok(false);
    }

    *lynch_opportunities = (*lynch_opportunities).saturating_sub(1);
    tracing::info!("Updated lynch opportunities to {lynch_opportunities}");

    Ok(true)
}

fn create_lynch_message(lyncher: &User, victim: &User) -> Result<CreateMessage> {
    Ok(CreateMessage::new()
        .content(
            MessageBuilder::new()
                .push(LYNCH_KNOWN_MESSAGE_PORTION)
                .mention(victim)
                .push(format!(
                    "? ({} {}'s needed)\n",
                    LYNCH_REQUIRED_REACTION_COUNT, LYNCH_YES_REACTION,
                ))
                .push(format!(
                    "Or, vote {} to lynch the author: ||",
                    LYNCH_NO_REACTION
                ))
                .mention(lyncher)
                .push("||\n")
                .push("Otherwise, this will be deleted ")
                .push(
                    (chrono::Utc::now() + Duration::from_secs(LYNCH_VOTING_SECONDS))
                        .discord_relative_timestamp(),
                )
                .build(),
        )
        .reactions([LYNCH_YES_REACTION, LYNCH_NO_REACTION]))
}

/// Lynch a user if you get 6 yay votes, get lynched yourself if they vote nay
#[poise::command(slash_command, rename = "lynch", ephemeral = true)]
pub async fn lynch(ctx: PoiseContext<'_>, victim: User) -> Result<()> {
    let lyncher = ctx.author();
    let guild_id = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let channel_id = ctx.channel_id();
    let react_role_id = ctx.data().config.read().await.bot_react_role_id;

    if !victim.has_role(ctx, guild_id, react_role_id).await? {
        ctx.say("You can't lynch a non reactme user!").await?;
        return Ok(());
    }

    if !check_lynch_opportunities().await? {
        ctx.say("No more lynch opportunities available").await?;
        return Ok(());
    }

    let msg = create_lynch_message(lyncher, &victim)?;

    let Ok(msg) = channel_id.send_message(ctx, msg).await else {
        ctx.say("Couldn't send message announcing lynching").await?;
        bail!("Couldn't send message announcing lynching");
    };

    LYNCH_MAP.insert(
        msg.id,
        LynchData {
            lyncher: lyncher.id,
            victim: victim.id,
            guild_id,
            channel_id,
        },
    );

    ctx.say("Lynching started!").await?;

    sleep(Duration::from_secs(LYNCH_VOTING_SECONDS)).await;

    if LYNCH_MAP.remove(&msg.id).is_some() {
        msg.delete(ctx).await.ok();
    }

    Ok(())
}

pub async fn update_interval() {
    use futures::StreamExt;

    // Every 1 hour, add a lynch opportunity up to the default, tokio interval
    IntervalStream::new(interval(Duration::from_secs(LYNCH_REFRESH_CHARGE_SECONDS)))
        .for_each(|_| async {
            let mut lynch_opportunities = LYNCH_OPPORTUNITIES.lock().await;
            *lynch_opportunities = (*lynch_opportunities + 1).min(LYNCH_DEFAULT_OPPORTUNITIES);
            tracing::trace!("Updated lynch opportunities to {lynch_opportunities}");
        })
        .await
}

async fn get_unique_non_kingfisher_voters(
    ctx: &serenity::Context,
    message: &serenity::Message,
    reaction: impl Into<serenity::ReactionType>,
) -> Result<Vec<User>> {
    let kingfisher_id = ctx.cache.current_user().id;

    Ok(message
        .reaction_users(ctx, reaction, None, None)
        .await?
        .into_iter()
        .filter(|user| user.id != kingfisher_id)
        .collect_vec())
}

// Handle a reaction
pub async fn handle_lynching(
    ctx: &serenity::Context,
    data: &AppState,
    message: &serenity::Message,
) -> Result<()> {
    let message_id = message.id;

    // check if message is in the lynch map
    let lynch_data = match LYNCH_MAP.get(&message_id) {
        Some(data) => data.clone(),
        None => return Ok(()),
    };

    let mut did_yay = 0;
    let mut did_nay = 0;

    for reaction in &message.reactions {
        if let serenity::ReactionType::Unicode(emoji) = &reaction.reaction_type {
            let char = emoji.chars().next().unwrap_or(' ');

            if char == LYNCH_YES_REACTION {
                did_yay += reaction.count;
            } else if char == LYNCH_NO_REACTION {
                did_nay += reaction.count;
            }
        }
    }

    let did_yay = did_yay >= LYNCH_REQUIRED_REACTION_COUNT;
    let did_nay = did_nay >= LYNCH_REQUIRED_REACTION_COUNT;

    if !did_yay && !did_nay {
        return Ok(());
    }

    // Make sure we don't count too many times
    if LYNCH_MAP.remove(&message_id).is_none() {
        return Ok(());
    }

    // This are costly api calls.
    let yay = get_unique_non_kingfisher_voters(ctx, message, LYNCH_YES_REACTION).await?;
    let nay = get_unique_non_kingfisher_voters(ctx, message, LYNCH_NO_REACTION).await?;

    // Delete the voting message
    message.delete(ctx).await.ok(); // Don't care if it succeeds

    let (target, shooters) = if did_yay {
        (&lynch_data.victim, yay)
    } else {
        (&lynch_data.lyncher, nay)
    };

    let time = std::time::Duration::from_secs(LYNCH_DURATION_SECONDS);
    let timeout_end = chrono::Utc::now() + time;

    save_to_lynch_leaderboard(ctx, data, target).await.ok();

    if lynch_data
        .guild_id
        .edit_member(
            ctx,
            target,
            serenity::EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
        )
        .await
        .is_err()
    {
        lynch_data
            .channel_id
            .send_message(
                ctx,
                CreateMessage::new().content(
                    MessageBuilder::new()
                        .push(format!(
                            "Sorry {}, but I couldn't lynch {}. Shame them publicly instead.",
                            shooters.mention_all(),
                            target.mention()
                        ))
                        .build(),
                ),
            )
            .await?;

        return Ok(());
    };

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

async fn save_to_lynch_leaderboard(
    ctx: &serenity::Context,
    data: &AppState,
    target: &UserId,
) -> Result<()> {
    let target = target.to_user(ctx).await?.id;
    let lynch_leaderboard = get_lynch_leaderboard(&data.db)?;

    lynch_leaderboard.increment(target)?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LynchEntry {
    user_id: serenity::UserId,
    count: u64,
}

impl PartialOrd for LynchEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LynchEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.count.cmp(&other.count)
    }
}

/// See who has been lynched the most
#[poise::command(slash_command, rename = "lynch_leaderboard", ephemeral = true)]
pub async fn lynch_leaderboard(ctx: PoiseContext<'_>) -> Result<()> {
    let mut message_text = String::from("### Lynch leaderboard:\n");
    let mut lynched = BinaryHeap::new();

    let lynch_leaderboard = get_lynch_leaderboard(&ctx.data().db)?;

    for (user_id, count) in lynch_leaderboard.iter() {
        lynched.push(LynchEntry { user_id, count });
    }

    for entry in lynched {
        message_text.push_str(&format!("{}: {}\n", entry.user_id.mention(), entry.count));
    }

    ctx.say(message_text).await?;

    Ok(())
}

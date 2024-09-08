use crate::{
    data::{AppState, PoiseContext},
    utils::GetRelativeTimestamp,
};
use bot_db::yeet::YeetLeaderboard;
use color_eyre::eyre::{bail, OptionExt, Result};
use core::str;
use itertools::Itertools;
use parking_lot::Mutex;
use poise::serenity_prelude::{
    self as serenity, ChannelId, CreateMessage, EditMessage, GuildId, Mentionable, MessageBuilder,
    MessageId, User, UserId,
};
use std::{
    cmp::Reverse,
    collections::HashMap,
    sync::LazyLock,
    time::{Duration, Instant},
};
use tokio::time::{interval, sleep};
use tokio_stream::wrappers::IntervalStream;

#[derive(Clone)]
pub struct YeetData {
    yeeter: UserId,
    victim: UserId,
    guild_id: GuildId,
    channel_id: ChannelId,
    start_time: Instant,
}

pub const YEET_DEFAULT_OPPORTUNITIES: usize = 3;
pub const YEET_REQUIRED_REACTION_COUNT: u64 = 6;
pub const YEET_NO_REACTION: char = '❌';
pub const YEET_YES_REACTION: char = '✅';
pub const YEET_DURATION_SECONDS: u64 = 300;
pub const YEET_REFRESH_CHARGE_SECONDS: u64 = 3600;
pub const YEET_VOTING_SECONDS: u64 = 90;
pub const YEET_KNOWN_MESSAGE_PORTION: &str = "Do you want to yeet ";

pub(crate) static YEET_MAP: LazyLock<Mutex<HashMap<MessageId, YeetData>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
pub(crate) static YEET_OPPORTUNITIES: LazyLock<Mutex<usize>> =
    LazyLock::new(|| Mutex::new(YEET_DEFAULT_OPPORTUNITIES));

async fn check_yeet_opportunities() -> Result<bool> {
    let mut yeet_opportunities = YEET_OPPORTUNITIES.lock();

    if *yeet_opportunities == 0 {
        return Ok(false);
    }

    *yeet_opportunities = (*yeet_opportunities).saturating_sub(1);
    tracing::info!("Updated yeet opportunities to {yeet_opportunities}");

    Ok(true)
}

fn create_yeet_message(yeeter: &User, victim: &User) -> Result<CreateMessage> {
    Ok(CreateMessage::new()
        .content(
            MessageBuilder::new()
                .push(YEET_KNOWN_MESSAGE_PORTION)
                .mention(victim)
                .push(format!(
                    "? ({} {}'s needed)\n",
                    YEET_REQUIRED_REACTION_COUNT, YEET_YES_REACTION,
                ))
                .push(format!(
                    "Or, vote {} to yeet the author: ||",
                    YEET_NO_REACTION
                ))
                .mention(yeeter)
                .push("||\n")
                .push("Otherwise, this will be deleted ")
                .push(
                    (chrono::Utc::now() + Duration::from_secs(YEET_VOTING_SECONDS))
                        .discord_relative_timestamp(),
                )
                .build(),
        )
        .reactions([YEET_YES_REACTION, YEET_NO_REACTION]))
}

/// Yeet a user if you get 6 yay votes, get yeeted yourself if they vote nay
#[poise::command(slash_command, rename = "yeet", ephemeral = true)]
pub async fn yeet(ctx: PoiseContext<'_>, victim: User) -> Result<()> {
    let yeeter = ctx.author();
    let guild_id = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let channel_id = ctx.channel_id();
    let react_role_id = ctx.data().config.read().await.bot_react_role_id;

    if !victim.has_role(ctx, guild_id, react_role_id).await? {
        ctx.say("You can't yeet a non reactme user!").await?;
        return Ok(());
    }

    if !check_yeet_opportunities().await? {
        ctx.say("No more yeet opportunities available").await?;
        return Ok(());
    }

    let msg = create_yeet_message(yeeter, &victim)?;

    let Ok(msg) = channel_id.send_message(ctx, msg).await else {
        ctx.say("Couldn't send message announcing yeeting").await?;
        bail!("Couldn't send message announcing yeeting");
    };
    let start_time = Instant::now();

    YEET_MAP.lock().insert(
        msg.id,
        YeetData {
            yeeter: yeeter.id,
            victim: victim.id,
            guild_id,
            channel_id,
            start_time,
        },
    );

    ctx.say("Yeeting started!").await?;

    sleep(Duration::from_secs(YEET_VOTING_SECONDS)).await;

    if YEET_MAP.lock().remove(&msg.id).is_some() {
        msg.delete(ctx).await.ok();
    }

    Ok(())
}

pub async fn update_interval() {
    use futures::StreamExt;

    // Every 1 hour, add a yeet opportunity up to the default, tokio interval
    IntervalStream::new(interval(Duration::from_secs(YEET_REFRESH_CHARGE_SECONDS)))
        .for_each(|_| async {
            let mut yeet_opportunities = YEET_OPPORTUNITIES.lock();
            *yeet_opportunities = (*yeet_opportunities + 1).min(YEET_DEFAULT_OPPORTUNITIES);
            tracing::trace!("Updated yeet opportunities to {yeet_opportunities}");
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
pub async fn handle_yeeting(
    ctx: &serenity::Context,
    data: &AppState,
    message: &serenity::Message,
) -> Result<()> {
    let message_id = message.id;

    // check if message is in the yeet map
    let yeet_data = match YEET_MAP.lock().get(&message_id) {
        Some(data) => data.clone(),
        None => return Ok(()),
    };

    let mut did_yay = 0;
    let mut did_nay = 0;

    for reaction in &message.reactions {
        if let serenity::ReactionType::Unicode(emoji) = &reaction.reaction_type {
            let char = emoji.chars().next().unwrap_or(' ');

            if char == YEET_YES_REACTION {
                did_yay += reaction.count;
            } else if char == YEET_NO_REACTION {
                did_nay += reaction.count;
            }
        }
    }

    let did_yay = did_yay >= YEET_REQUIRED_REACTION_COUNT;
    let did_nay = did_nay >= YEET_REQUIRED_REACTION_COUNT;

    if !did_yay && !did_nay {
        return Ok(());
    }

    // Make sure we don't count too many times
    if YEET_MAP.lock().remove(&message_id).is_none() {
        return Ok(());
    }

    // This are costly api calls.
    let yay = get_unique_non_kingfisher_voters(ctx, message, YEET_YES_REACTION).await?;
    let nay = get_unique_non_kingfisher_voters(ctx, message, YEET_NO_REACTION).await?;

    // Delete the voting message
    message.delete(ctx).await.ok(); // Don't care if it succeeds

    let (target, shooters) = if did_yay {
        (&yeet_data.victim, yay)
    } else {
        (&yeet_data.yeeter, nay)
    };

    let time = std::time::Duration::from_secs(YEET_DURATION_SECONDS);
    let timeout_end = chrono::Utc::now() + time;

    save_to_yeet_leaderboard(ctx, data, target).await.ok();

    if yeet_data
        .guild_id
        .edit_member(
            ctx,
            target,
            serenity::EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
        )
        .await
        .is_err()
    {
        yeet_data
            .channel_id
            .send_message(
                ctx,
                CreateMessage::new().content(
                    MessageBuilder::new()
                        .push(format!(
                            "Sorry {}, but I couldn't yeet {}. Shame them publicly instead.",
                            shooters.mention_all(),
                            target.mention()
                        ))
                        .build(),
                ),
            )
            .await?;

        return Ok(());
    };

    let duration = yeet_data.start_time.elapsed();

    let mut message_handle = yeet_data
        .channel_id
        .send_message(
            ctx,
            CreateMessage::new().content(
                MessageBuilder::new()
                    .push(format!(
                        "User {} has been yeeted in {} seconds! They will return {}\nBrought to you by: {}",
                        target.mention(),
                        duration.as_secs(),
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
                "User {} was yeeted in {} seconds\nBrought to you by: {}",
                target.mention(),
                duration.as_secs(),
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

async fn save_to_yeet_leaderboard(
    ctx: &serenity::Context,
    data: &AppState,
    target: &UserId,
) -> Result<()> {
    let target = target.to_user(ctx).await?.id;
    YeetLeaderboard::new(&data.db)?.increment(target)?;

    Ok(())
}

/// See who has been yeeted the most
#[poise::command(slash_command, rename = "yeeterboard", ephemeral = true)]
pub async fn yeet_leaderboard(ctx: PoiseContext<'_>) -> Result<()> {
    let mut message_text = String::from("### Yeet leaderboard:\n");

    let yeet_leaderboard = YeetLeaderboard::new(&ctx.data().db)?;

    for (user_id, count) in yeet_leaderboard
        .iter()
        .sorted_by_key(|(_, count)| Reverse(*count))
    {
        message_text.push_str(&format!("{}: {}\n", user_id.mention(), count));
    }

    ctx.say(message_text).await?;

    Ok(())
}

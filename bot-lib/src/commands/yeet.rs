use crate::{
    data::{AppState, PoiseContext},
    utils::GetRelativeTimestamp,
    CloneableCtx, IntoCloneableCtx, MentionableExt, TimeoutExt,
};
use bot_db::yeet::YeetLeaderboard;
use bot_traits::ForwardRefToTracing;
use chrono::{DateTime, Utc};
use color_eyre::eyre::{bail, OptionExt, Result};
use futures::StreamExt;
use itertools::Itertools;
use parking_lot::Mutex;
use poise::serenity_prelude::{
    CacheHttp, ChannelId, Context, CreateMessage, EditMessage, GuildId, Mentionable, Message,
    MessageBuilder, MessageId, ReactionType, User, UserId,
};
use rand::Rng;
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
    is_yeet_amongus_easter_egg: bool,
}

pub const YEET_DEFAULT_OPPORTUNITIES: usize = 3;
pub const YEET_REQUIRED_REACTION_COUNT: u64 = 6;
pub const YEET_NO_REACTION: char = '❌';
pub const YEET_YES_REACTION: char = '✅';
pub const YEET_DURATION_SECONDS: u64 = 300;
pub const YEET_REFRESH_CHARGE_SECONDS: u64 = 3600;
pub const YEET_VOTING_SECONDS: u64 = 90;

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

fn create_yeet_message(
    yeeter: &User,
    victim: &User,
    ctx: CloneableCtx,
    channel_id: ChannelId,
    is_yeet_amongus_easter_egg: bool,
) -> Result<CreateMessage> {
    if is_yeet_amongus_easter_egg {
        create_yeet_message_easter_egg(yeeter, victim, ctx, channel_id)
    } else {
        create_yeet_message_basic(yeeter, victim)
    }
}

fn create_yeet_message_basic(yeeter: &User, victim: &User) -> Result<CreateMessage> {
    Ok(CreateMessage::new()
        .content(
            MessageBuilder::new()
                .push("Do you want to yeet ")
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

async fn meeting_message(ctx: CloneableCtx, channel_id: ChannelId) -> Result<()> {
    let message = channel_id
        .send_message(
            &ctx,
            CreateMessage::new().content(
                "https://tenor.com/view/emergency-meeting-among-us-meeting-discuss-gif-18383222",
            ),
        )
        .await?;

    tokio::time::sleep(Duration::from_secs(5)).await;

    message.delete(ctx).await?;

    Ok(())
}

fn create_yeet_message_easter_egg(
    yeeter: &User,
    victim: &User,
    ctx: CloneableCtx,
    channel_id: ChannelId,
) -> Result<CreateMessage> {
    tokio::spawn(async move {
        meeting_message(ctx, channel_id).await.ok();
    });

    Ok(CreateMessage::new()
        .content(
            MessageBuilder::new()
                .push("Is ")
                .mention(victim)
                .push("the impostor?\n")
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

#[tracing::instrument(level = "trace", skip(ctx, guild_id))]
pub async fn can_yeet(ctx: PoiseContext<'_>, victim: &User, guild_id: GuildId) -> Result<()> {
    let react_role_id = ctx.data().config.read().await.bot_react_role_id;

    if !victim.has_role(ctx, guild_id, react_role_id).await? {
        ctx.say("You can't yeet a non reactme user!").await?;
        return Ok(());
    }

    if !check_yeet_opportunities().await? {
        ctx.say("No more yeet opportunities available").await?;
        return Ok(());
    }
    Ok(())
}

/// Yeet a user if you get 6 yay votes, get yeeted yourself if they vote nay
#[poise::command(slash_command, rename = "yeet", ephemeral = true)]
pub async fn yeet(ctx: PoiseContext<'_>, victim: User) -> Result<()> {
    let yeeter = ctx.author();
    let guild_id = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let channel_id = ctx.channel_id();

    can_yeet(ctx, &victim, guild_id).await?;

    let is_yeet_amongus_easter_egg = rand::thread_rng().gen_bool(0.01);

    let msg = create_yeet_message(
        yeeter,
        &victim,
        (&ctx).into(),
        channel_id,
        is_yeet_amongus_easter_egg,
    )?;

    let Ok(msg) = channel_id.send_message(ctx, msg).await else {
        ctx.say("Couldn't send message announcing yeeting").await?;
        bail!("Couldn't send message announcing yeeting");
    };

    YEET_MAP.lock().insert(
        msg.id,
        YeetData {
            yeeter: yeeter.id,
            victim: victim.id,
            guild_id,
            channel_id,
            start_time: Instant::now(),
            is_yeet_amongus_easter_egg,
        },
    );

    ctx.say("Yeeting started!").await?;

    sleep(Duration::from_secs(YEET_VOTING_SECONDS)).await;

    if YEET_MAP.lock().remove(&msg.id).is_some() {
        msg.delete(ctx).await.ok();
    }

    if is_yeet_amongus_easter_egg {
        easter_egg_failure(ctx, channel_id).await.ok();
    }

    Ok(())
}

async fn easter_egg_failure(ctx: impl CacheHttp, channel_id: ChannelId) -> Result<()> {
    channel_id
    .send_message(
        ctx,
        CreateMessage::new().content(
            "https://cdn.discordapp.com/attachments/1065374082373271655/1286171602525880350/No_one_was_ejected.gif?ex=66ecf025&is=66eb9ea5&hm=ea740fe207b75c6c27852bae83e9724377cfe08d5f523653bf0c6b3cf82b7232&",
        ),
    )
    .await.ok();
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
    ctx: &Context,
    message: &Message,
    reaction: impl Into<ReactionType>,
) -> Result<Vec<User>> {
    let kingfisher_id = ctx.cache.current_user().id;

    Ok(message
        .reaction_users(ctx, reaction, None, None)
        .await?
        .into_iter()
        .filter(|user| user.id != kingfisher_id)
        .collect_vec())
}

async fn fail_to_yeet_after_vote(
    ctx: CloneableCtx,
    channel_id: ChannelId,
    is_yeet_amongus_easter_egg: bool,
    shooters: &[User],
    target: &UserId,
) -> Result<()> {
    if is_yeet_amongus_easter_egg {
        easter_egg_failure(ctx, channel_id).await?;
    } else {
        channel_id
            .send_message(
                ctx,
                CreateMessage::new().content(format!(
                    "Sorry {}, but I couldn't yeet {}. Shame them publicly instead.",
                    shooters.mention_all(),
                    target.mention()
                )),
            )
            .await?;
    }

    Ok(())
}

async fn successful_yeet(
    ctx: CloneableCtx,
    channel_id: ChannelId,
    is_yeet_amongus_easter_egg: bool,
    shooters: &[User],
    target: &UserId,
    duration: Duration,
    timeout_end: DateTime<Utc>,
) -> Result<()> {
    if is_yeet_amongus_easter_egg {
        channel_id
            .send_message(
                &ctx,
                CreateMessage::new()
                    .content("https://tenor.com/view/among-us-ejected-impostor-gif-19787031"),
            )
            .await?;
    }

    let verb = if is_yeet_amongus_easter_egg {
        "ejected"
    } else {
        "yeeted"
    };

    let mut message_handle = channel_id
        .send_message(
            &ctx,
            CreateMessage::new().content(format!(
                "User {} has been {} in {}.{} seconds! They will return {}\nBrought to you by: {}",
                target.mention(),
                verb,
                duration.as_secs(),
                duration.subsec_millis(),
                timeout_end.discord_relative_timestamp(),
                shooters.mention_all(),
            )),
        )
        .await?;

    tokio::time::sleep(Duration::from_secs(YEET_DURATION_SECONDS - 1)).await;

    message_handle
        .edit(
            ctx,
            EditMessage::new().content(format!(
                "User {} was {} in {}.{} seconds\nBrought to you by: {}",
                target.mention(),
                verb,
                duration.as_secs(),
                duration.subsec_millis(),
                shooters.mention_all()
            )),
        )
        .await?;

    Ok(())
}

fn should_yeet_someone(message: &Message) -> Option<(YeetData, bool, bool)> {
    let mut did_yay = 0;
    let mut did_nay = 0;

    for reaction in &message.reactions {
        if let ReactionType::Unicode(emoji) = &reaction.reaction_type {
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

    (did_yay || did_nay)
        .then(|| YEET_MAP.lock().remove(&message.id))
        .flatten()
        .map(|data| (data, did_yay, did_nay))
}

// Handle a reaction
pub async fn handle_yeeting(ctx: &Context, data: &AppState, message: &Message) -> Result<()> {
    let Some((yeet_data, did_yay, did_nay)) = should_yeet_someone(message) else {
        return Ok(());
    };

    let duration = yeet_data.start_time.elapsed();

    // This are costly api calls.
    let yay = get_unique_non_kingfisher_voters(ctx, message, YEET_YES_REACTION).await?;
    let nay = get_unique_non_kingfisher_voters(ctx, message, YEET_NO_REACTION).await?;

    // Delete the voting message
    message.delete(ctx).await.ok(); // Don't care if it succeeds

    let (targets, shooters): (&[UserId], Vec<User>) = match (did_yay, did_nay) {
        (true, true) => {
            yeet_data
                .channel_id
                .send_message(
                    ctx,
                    CreateMessage::new()
                        .content("Whoops. Discord decided to be bad and didn't allow KingFisher to yeet only one. How about two? :)"),
                )
                .await
                .ok();

            (
                &[yeet_data.victim, yeet_data.yeeter],
                [yay, nay].into_iter().flatten().unique().collect_vec(),
            )
        }
        (true, false) => (&[yeet_data.victim], yay),
        (false, true) => (&[yeet_data.yeeter], nay),
        (false, false) => {
            tracing::error!("Yeet failure in counting? This should never happen");
            return Ok(());
        }
    };

    save_to_yeet_leaderboard(data, targets).trace_err_ok();

    futures::stream::iter(targets)
        .for_each_concurrent(None, |target| async {
            let ctx = ctx.get_cloneable_ctx();

            match yeet_data
                .guild_id
                .timeout(&ctx, target, Duration::from_secs(YEET_DURATION_SECONDS))
                .await
            {
                Ok((_, timeout_end)) => successful_yeet(
                    ctx,
                    yeet_data.channel_id,
                    yeet_data.is_yeet_amongus_easter_egg,
                    &shooters,
                    target,
                    duration,
                    timeout_end,
                )
                .await
                .trace_err_ok(),

                _ => fail_to_yeet_after_vote(
                    ctx,
                    yeet_data.channel_id,
                    yeet_data.is_yeet_amongus_easter_egg,
                    &shooters,
                    target,
                )
                .await
                .trace_err_ok(),
            };
        })
        .await;

    Ok(())
}

fn save_to_yeet_leaderboard(data: &AppState, targets: &[UserId]) -> Result<()> {
    for &target in targets {
        YeetLeaderboard::new(&data.db)?.increment(target)?;
    }

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
        message_text.push_str(&format!("{}: {}\t", user_id.mention(), count));
    }

    ctx.say(message_text).await?;

    Ok(())
}

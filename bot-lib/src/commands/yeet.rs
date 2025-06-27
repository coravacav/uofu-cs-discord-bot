use crate::{
    MentionableExt, TimeoutExt,
    data::{PoiseContext, State},
    utils::GetRelativeTimestamp,
};
use bot_db::yeet::YeetLeaderboard;
use bot_traits::ForwardRefToTracing;
use chrono::{DateTime, Utc};
use color_eyre::eyre::{OptionExt, Result, bail};
use itertools::Itertools;
use parking_lot::Mutex;
use poise::serenity_prelude::{
    ChannelId, Context, CreateMessage, EditMessage, GuildId, Mentionable, Message, MessageBuilder,
    MessageId, ReactionType, User, UserId,
};
use rand::Rng;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    cmp::Reverse,
    num::Saturating,
    sync::{Arc, LazyLock},
    time::{Duration, Instant},
};
use tokio::{
    join,
    time::{interval, sleep},
};
use tokio_stream::wrappers::IntervalStream;

#[derive(Clone)]
pub struct YeetContext {
    yeeter: UserId,
    victim: UserId,
    guild_id: GuildId,
    channel_id: ChannelId,
    message_id: MessageId,
    start_time: Instant,
    is_yeet_amongus_easter_egg: bool,
}

impl YeetContext {
    fn new(
        yeeter: UserId,
        victim: UserId,
        guild_id: GuildId,
        channel_id: ChannelId,
        message_id: MessageId,
        is_yeet_amongus_easter_egg: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            yeeter,
            victim,
            guild_id,
            channel_id,
            message_id,
            start_time: Instant::now(),
            is_yeet_amongus_easter_egg,
        })
    }
}

pub const YEET_DEFAULT_OPPORTUNITIES: Saturating<usize> = Saturating(3);
pub const YEET_REQUIRED_REACTION_COUNT: u64 = 6;
pub const YEET_NO_REACTION: char = '❌';
pub const YEET_YES_REACTION: char = '✅';
pub const YEET_DURATION_SECONDS: u64 = 300;
pub const YEET_REFRESH_CHARGE_SECONDS: u64 = 3600;
pub const YEET_VOTING_SECONDS: u64 = 90;
pub const YEET_PARRY_SECONDS: u64 = 5;
pub const YEET_PARRY_COOLDOWN_SECONDS: u64 = 60;

pub(crate) static YEET_MAP: LazyLock<Mutex<FxHashMap<MessageId, Arc<YeetContext>>>> =
    LazyLock::new(|| Mutex::new(FxHashMap::default()));
pub(crate) static YEET_STARBOARD_EXCLUSIONS: LazyLock<Mutex<FxHashSet<MessageId>>> =
    LazyLock::new(|| Mutex::new(FxHashSet::default()));
pub(crate) static YEET_OPPORTUNITIES: LazyLock<Mutex<Saturating<usize>>> =
    LazyLock::new(|| Mutex::new(YEET_DEFAULT_OPPORTUNITIES));
pub(crate) static YEET_PARRY_MAP: LazyLock<Mutex<FxHashMap<UserId, (Instant, u64)>>> =
    LazyLock::new(|| Mutex::new(FxHashMap::default()));

fn has_yeet_opportunities() -> bool {
    let mut yeet_opportunities = YEET_OPPORTUNITIES.lock();

    if yeet_opportunities.0 > 0 {
        *yeet_opportunities -= 1;
        true
    } else {
        false
    }
}

fn create_yeet_message(
    yeeter: &User,
    victim: &User,
    ctx: &Context,
    channel_id: ChannelId,
    is_yeet_amongus_easter_egg: bool,
) -> Result<CreateMessage> {
    let time = (chrono::Utc::now() + Duration::from_secs(YEET_VOTING_SECONDS))
        .discord_relative_timestamp();

    let message_content = if is_yeet_amongus_easter_egg {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            meeting_message(ctx, channel_id).await.ok();
        });

        MessageBuilder::new()
            .push("Is ")
            .mention(victim)
            .push("the impostor?\n")
            .push(format!(
                "Or, vote {YEET_NO_REACTION} to yeet the author: ||"
            ))
            .mention(yeeter)
            .push("||\n")
            .push("Otherwise, this will be deleted ")
            .push(time)
            .build()
    } else {
        MessageBuilder::new()
            .push("Do you want to yeet ")
            .mention(victim)
            .push(format!(
                "? ({YEET_REQUIRED_REACTION_COUNT} {YEET_YES_REACTION}'s needed)\n",
            ))
            .push(format!(
                "Or, vote {YEET_NO_REACTION} to yeet the author: ||"
            ))
            .mention(yeeter)
            .push("||\n")
            .push("Otherwise, this will be deleted ")
            .push(time)
            .build()
    };

    Ok(CreateMessage::new()
        .content(message_content)
        .reactions([YEET_YES_REACTION, YEET_NO_REACTION]))
}

async fn meeting_message(ctx: Context, channel_id: ChannelId) -> Result<()> {
    let message = channel_id
        .send_message(
            &ctx,
            CreateMessage::new().content(
                "https://tenor.com/view/emergency-meeting-among-us-meeting-discuss-gif-18383222",
            ),
        )
        .await?;

    tokio::spawn(async move {
        sleep(Duration::from_secs(5)).await;
        message.delete(ctx).await.trace_err_ok();
    });

    Ok(())
}

pub async fn can_yeet(ctx: PoiseContext<'_>, victim: &User, guild_id: GuildId) -> Result<bool> {
    let react_role_id = ctx.data().config.read().await.ids.bot_react_role_id;

    if !victim.has_role(ctx, guild_id, react_role_id).await? {
        ctx.say("You can't yeet a non reactme user!").await?;
        return Ok(false);
    }

    if !has_yeet_opportunities() {
        ctx.say("No more yeet opportunities available").await?;
        return Ok(false);
    }

    Ok(true)
}

/// Yeet a user if you get 6 yay votes, get yeeted yourself if they vote nay
#[poise::command(slash_command, rename = "yeet", ephemeral = true)]
pub async fn yeet(ctx: PoiseContext<'_>, victim: User) -> Result<()> {
    let yeeter = ctx.author();
    let guild_id = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let channel_id = ctx.channel_id();

    if !can_yeet(ctx, &victim, guild_id).await? {
        return Ok(());
    }

    let is_yeet_amongus_easter_egg = rand::rng().random_bool(0.02);

    let msg = create_yeet_message(
        yeeter,
        &victim,
        ctx.serenity_context(),
        channel_id,
        is_yeet_amongus_easter_egg,
    )?;

    let Ok(msg) = channel_id.send_message(ctx, msg).await else {
        ctx.say("Couldn't send message announcing yeeting").await?;
        bail!("Couldn't send message announcing yeeting");
    };

    YEET_MAP.lock().insert(
        msg.id,
        YeetContext::new(
            yeeter.id,
            victim.id,
            guild_id,
            channel_id,
            msg.id,
            is_yeet_amongus_easter_egg,
        ),
    );

    YEET_STARBOARD_EXCLUSIONS.lock().insert(msg.id);

    ctx.say("Yeeting started!").await?;

    sleep(Duration::from_secs(YEET_VOTING_SECONDS)).await;

    if YEET_MAP.lock().remove(&msg.id).is_some() {
        tokio::spawn({
            let ctx = ctx.serenity_context().clone();
            async move { msg.delete(ctx).await }
        });

        if is_yeet_amongus_easter_egg {
            easter_egg_failure(ctx.serenity_context(), channel_id)
                .await
                .ok();
        }
    }

    Ok(())
}

async fn easter_egg_failure(ctx: &Context, channel_id: ChannelId) -> Result<()> {
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
            *yeet_opportunities += 1;
            *yeet_opportunities = yeet_opportunities.min(YEET_DEFAULT_OPPORTUNITIES);
        })
        .await
}

async fn get_unique_non_kingfisher_voters(
    ctx: Context,
    yeet_context: &YeetContext,
    reaction: impl Into<ReactionType>,
) -> Result<Arc<[UserId]>> {
    let kingfisher_id = ctx.cache.current_user().id;

    Ok(yeet_context
        .channel_id
        .reaction_users(ctx, yeet_context.message_id, reaction, None, None)
        .await?
        .into_iter()
        .filter(|user| user.id != kingfisher_id)
        .map(|user| user.id)
        .collect())
}

async fn fail_to_yeet_after_vote(
    ctx: Context,
    yeet_context: &YeetContext,
    shooters: &[UserId],
    target: &UserId,
) -> Result<()> {
    if yeet_context.is_yeet_amongus_easter_egg {
        easter_egg_failure(&ctx, yeet_context.channel_id).await?;
    } else {
        yeet_context
            .channel_id
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
    ctx: Context,
    yeet_context: &YeetContext,
    shooters: &[UserId],
    target: &UserId,
    duration: Duration,
    timeout_end: DateTime<Utc>,
) -> Result<()> {
    if yeet_context.is_yeet_amongus_easter_egg {
        yeet_context
            .channel_id
            .send_message(
                &ctx,
                CreateMessage::new()
                    .content("https://tenor.com/view/among-us-ejected-impostor-gif-19787031"),
            )
            .await?;
    }

    let verb = if yeet_context.is_yeet_amongus_easter_egg {
        "ejected"
    } else {
        "yeeted"
    };

    let mut message_handle = yeet_context
        .channel_id
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

fn should_yeet_someone(message: &Message) -> Option<(Arc<YeetContext>, bool, bool)> {
    let mut did_yay = 0;
    let mut did_nay = 0;

    for reaction in &message.reactions {
        if let ReactionType::Unicode(emoji) = &reaction.reaction_type {
            match emoji.chars().next() {
                Some(YEET_YES_REACTION) => {
                    did_yay += reaction.count;
                }
                Some(YEET_NO_REACTION) => {
                    did_nay += reaction.count;
                }
                _ => {}
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
pub async fn handle_yeeting(ctx: &Context, data: State, message: &Message) -> Result<()> {
    let Some((yeet_context, did_yay, did_nay)) = should_yeet_someone(message) else {
        return Ok(());
    };

    let duration = yeet_context.start_time.elapsed();
    let current_instant = Instant::now();
    let parried = YEET_PARRY_MAP
        .lock()
        .remove(&yeet_context.victim)
        .map(|(parry_time, attempts)| {
            (current_instant - parry_time).as_secs() < YEET_PARRY_SECONDS && attempts == 0
        })
        .unwrap_or(false);

    // This are costly api calls.

    let (yay, nay) = join!(
        tokio::spawn({
            let ctx = ctx.clone();
            let yeet_context = yeet_context.clone();
            async move {
                get_unique_non_kingfisher_voters(ctx, &yeet_context, YEET_YES_REACTION)
                    .await
                    .trace_err_ok()
            }
        }),
        tokio::spawn({
            let ctx = ctx.clone();
            let yeet_context = yeet_context.clone();
            async move {
                get_unique_non_kingfisher_voters(ctx, &yeet_context, YEET_NO_REACTION)
                    .await
                    .trace_err_ok()
            }
        })
    );

    let (Ok(Some(yay)), Ok(Some(nay))) = (yay, nay) else {
        tracing::error!("Yeet failure in counting? This should never happen");
        return Ok(());
    };

    let parried = !yay.iter().any(|user| *user == yeet_context.victim) && parried;

    // Delete the voting message
    let channel_id = message.channel_id;
    let guild_id = yeet_context.guild_id;
    let message_id = message.id;
    tokio::spawn({
        let ctx = ctx.clone();
        async move {
            channel_id.delete_message(ctx, message_id).await.ok();
        }
    });

    let (targets, shooters): (&[UserId], Arc<[UserId]>) = match (did_yay, did_nay, parried) {
        (true, true, _) => {
            tokio::spawn({
                let ctx = ctx.clone();

                async move {
                    channel_id.send_message(
                        ctx,
                        CreateMessage::new().content("Whoops. Discord decided to be bad and didn't allow KingFisher to yeet only one. How about two? :)")
                    )
                    .await
                    .ok();
                }
            });

            (
                &[yeet_context.victim, yeet_context.yeeter],
                yay.iter().chain(nay.iter()).unique().cloned().collect(),
            )
        }
        (true, false, false) => (&[yeet_context.victim], yay),
        (true, false, true) => {
            tokio::spawn({
                let ctx = ctx.clone();
                let victim = yeet_context.victim.mention();
                let yeeter = yeet_context.yeeter.mention();
                async move {
                    channel_id
                        .send_message(
                            ctx,
                            CreateMessage::new().content(format!(
                                "{victim} has successfully parried the yeet! Take that {yeeter}!"
                            )),
                        )
                        .await
                        .ok();
                }
            });

            (
                &[yeet_context.yeeter],
                nay.iter()
                    .cloned()
                    .chain(Some(yeet_context.victim))
                    .unique()
                    .collect(),
            )
        }
        (false, true, _) => (&[yeet_context.yeeter], nay),
        (false, false, _) => {
            bail!("Yeet failure in counting? This should never happen");
        }
    };

    save_to_yeet_leaderboard(data, targets).trace_err_ok();

    for &target in targets {
        let shooters = shooters.clone();

        tokio::spawn({
            let ctx = ctx.clone();
            let yeet_context = yeet_context.clone();
            async move {
                match guild_id
                    .timeout(&ctx, &target, Duration::from_secs(YEET_DURATION_SECONDS))
                    .await
                {
                    Ok((_, timeout_end)) => successful_yeet(
                        ctx,
                        &yeet_context,
                        &shooters,
                        &target,
                        duration,
                        timeout_end,
                    )
                    .await
                    .trace_err_ok(),

                    _ => fail_to_yeet_after_vote(ctx, &yeet_context, &shooters, &target)
                        .await
                        .trace_err_ok(),
                };
            }
        });
    }

    Ok(())
}

fn save_to_yeet_leaderboard(data: State, targets: &[UserId]) -> Result<()> {
    for &target in targets {
        YeetLeaderboard::connect(&data.db)?.increment(target)?;
    }

    Ok(())
}

/// See who has been yeeted the most
#[poise::command(slash_command, rename = "yeeterboard", ephemeral = true)]
pub async fn yeet_leaderboard(ctx: PoiseContext<'_>) -> Result<()> {
    let mut message_text = String::from("### Yeet leaderboard:\n");

    let yeet_leaderboard = YeetLeaderboard::connect(&ctx.data().db)?;

    for (user_id, count) in yeet_leaderboard
        .iter()
        .sorted_by_key(|(_, count)| Reverse(*count))
    {
        message_text.push_str(&format!("{}: {}\t", user_id.mention(), count));
    }

    ctx.say(message_text).await?;

    Ok(())
}

/// Parry a yeet for 5 seconds.
/// If you do it more than once a minute, it will fail :)
#[poise::command(slash_command, ephemeral = true)]
pub async fn parry(ctx: PoiseContext<'_>) -> Result<()> {
    let user = ctx.author();

    YEET_PARRY_MAP
        .lock()
        .entry(user.id)
        .and_modify(|(last_time, attempts)| {
            if last_time.elapsed() < Duration::from_secs(YEET_PARRY_COOLDOWN_SECONDS) {
                *attempts += 1;
            } else {
                *last_time = Instant::now();
                *attempts = 0;
            }
        })
        .or_insert((Instant::now(), 0));

    ctx.say("You're now parrying for the next 5 seconds")
        .await?;

    Ok(())
}

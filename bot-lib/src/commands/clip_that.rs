use std::{
    future,
    sync::LazyLock,
    time::{Duration, Instant},
};

use crate::{SayThenDelete, data::PoiseContext};
use bot_traits::ForwardRefToTracing;
use color_eyre::eyre::Result;
use futures::StreamExt;
use itertools::Itertools;
use poise::serenity_prelude::{
    CreateAllowedMentions, CreateMessage, MessageBuilder, MessageId, Timestamp, UserId,
};
use regex::{Captures, Regex};
use rustc_hash::{FxHashMap, FxHashSet};
use tokio::sync::Mutex;

/// Take the last X mesasges and make a nice new message that contains them all.
#[poise::command(slash_command, ephemeral = true)]
pub async fn clip_that(
    ctx: PoiseContext<'_>,
    #[description = "How many messages to clip (up to 10)"] amount: usize,
) -> Result<()> {
    if amount > 10 {
        ctx.say("You can only clip up to 10 messages at a time.")
            .await?;
        return Ok(());
    }

    ctx.defer_ephemeral().await?;

    let user_id = ctx.author().id;

    static INSTANT_BY_USER_ID: LazyLock<parking_lot::Mutex<FxHashMap<UserId, Instant>>> =
        LazyLock::new(|| parking_lot::Mutex::new(FxHashMap::default()));

    let old_time = INSTANT_BY_USER_ID.lock().insert(user_id, Instant::now());

    if let Some(last_time) = old_time
        && last_time.elapsed() < Duration::from_secs(300)
    {
        ctx.say_then_delete("300 more seconds of no clipping :)")
            .await?;
        return Ok(());
    }

    let channel_id = ctx.channel_id();

    let mut new_message = MessageBuilder::new();

    new_message.mention(&user_id);
    new_message.push(" clipped chat!\n\n");

    let url_regex = Regex::new(
        r"((?:http[s]?://.)?(?:www\.)?[-a-zA-Z0-9@%._\+~#=]{2,256}\.[a-z]{2,6}\b(?:[-a-zA-Z0-9@:%_\+.~#?&//=]*))",
    )
    .unwrap();

    static MESSAGES_SENT_BY_THIS: LazyLock<Mutex<FxHashSet<Option<MessageId>>>> =
        LazyLock::new(|| Mutex::new(FxHashSet::default()));

    let mut mesages_sent_by_this = MESSAGES_SENT_BY_THIS.lock().await;

    let number_added = channel_id
        .messages_iter(ctx)
        .take(amount)
        .filter(|msg| {
            future::ready(
                !msg.as_ref()
                    .map(|msg| mesages_sent_by_this.contains(&Some(msg.id)))
                    .unwrap_or(false),
            )
        })
        .collect::<Vec<_>>()
        .await
        .iter()
        .flatten()
        .rev()
        .chunk_by(|msg| msg.author.id)
        .into_iter()
        .flat_map(|(author, messages)| {
            let mut time_string = String::new();
            let mut clips = String::new();
            for msg in messages {
                time_string.push_str(&format!("{}, ", get_nice_date(msg.timestamp)));

                if msg.content.is_empty() && !msg.attachments.is_empty() {
                    clips.push_str("(image)\n");
                } else {
                    clips.push_str(&format!(
                        "> {}\n",
                        url_regex.replace(&msg.content, |caps: &Captures| {
                            format!("<{}>", &caps[1])
                        })
                    ));
                }
            }

            if clips.is_empty() || time_string.is_empty() {
                return Err(());
            }

            time_string.pop();
            time_string.pop();

            new_message.mention(&author);
            new_message.push_safe(format!(" ({time_string})\n"));
            new_message.push_safe(clips);

            Ok(())
        })
        .count();

    if number_added == 0 {
        ctx.say_then_delete("No valid messages to clip :)").await?;
        return Ok(());
    }

    let message_id = channel_id
        .send_message(
            ctx,
            CreateMessage::new()
                .allowed_mentions(CreateAllowedMentions::new())
                .content(new_message.build()),
        )
        .await
        .trace_err_ok();

    mesages_sent_by_this.insert(message_id.map(|message| message.id));

    drop(mesages_sent_by_this);

    ctx.say_then_delete("Clip made!").await.trace_err_ok();

    Ok(())
}

fn get_nice_date(timestamp: Timestamp) -> String {
    format!("<t:{}:t>", timestamp.timestamp())
}

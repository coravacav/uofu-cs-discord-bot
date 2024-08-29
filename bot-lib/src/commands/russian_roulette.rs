// use crate::{
//     data::{AppState, PoiseContext},
//     db::get_yeet_leaderboard,
//     utils::GetRelativeTimestamp,
// };
// use color_eyre::eyre::{bail, OptionExt, Result};
// use core::str;
// use dashmap::DashMap;
// use itertools::Itertools;
// use poise::serenity_prelude::{
//     self as serenity, ChannelId, CreateMessage, EditMessage, GuildId, Mentionable, MessageBuilder,
//     MessageId, User, UserId,
// };
// use std::{collections::BinaryHeap, sync::LazyLock, time::Duration};
// use tokio::{
//     sync::Mutex,
//     time::{interval, sleep},
// };
// use tokio_stream::wrappers::IntervalStream;

// #[derive(Clone)]
// pub struct RussianRoulette {
//     yeeter: UserId,
//     victim: UserId,
//     guild_id: GuildId,
//     channel_id: ChannelId,
// }

// /// Start a round of russian roulette
// #[poise::command(slash_command, ephemeral = true)]
// pub async fn yeet(ctx: PoiseContext<'_>, victim: User) -> Result<()> {
//     let yeeter = ctx.author();
//     let guild_id = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
//     let channel_id = ctx.channel_id();
//     let react_role_id = ctx.data().config.read().await.bot_react_role_id;

//     if !victim.has_role(ctx, guild_id, react_role_id).await? {
//         ctx.say("You can't yeet a non reactme user!").await?;
//         return Ok(());
//     }

//     if !check_yeet_opportunities().await? {
//         ctx.say("No more yeet opportunities available").await?;
//         return Ok(());
//     }

//     let msg = create_yeet_message(yeeter, &victim)?;

//     let Ok(msg) = channel_id.send_message(ctx, msg).await else {
//         ctx.say("Couldn't send message announcing yeeting").await?;
//         bail!("Couldn't send message announcing yeeting");
//     };

//     YEET_MAP.insert(
//         msg.id,
//         RussianRoulette {
//             yeeter: yeeter.id,
//             victim: victim.id,
//             guild_id,
//             channel_id,
//         },
//     );

//     ctx.say("Yeeting started!").await?;

//     sleep(Duration::from_secs(YEET_VOTING_SECONDS)).await;

//     if YEET_MAP.remove(&msg.id).is_some() {
//         msg.delete(ctx).await.ok();
//     }

//     Ok(())
// }

// pub async fn update_interval() {
//     use futures::StreamExt;

//     // Every 1 hour, add a yeet opportunity up to the default, tokio interval
//     IntervalStream::new(interval(Duration::from_secs(YEET_REFRESH_CHARGE_SECONDS)))
//         .for_each(|_| async {
//             let mut yeet_opportunities = YEET_OPPORTUNITIES.lock().await;
//             *yeet_opportunities = (*yeet_opportunities + 1).min(YEET_DEFAULT_OPPORTUNITIES);
//             tracing::trace!("Updated yeet opportunities to {yeet_opportunities}");
//         })
//         .await
// }

// async fn get_unique_non_kingfisher_voters(
//     ctx: &serenity::Context,
//     message: &serenity::Message,
//     reaction: impl Into<serenity::ReactionType>,
// ) -> Result<Vec<User>> {
//     let kingfisher_id = ctx.cache.current_user().id;

//     Ok(message
//         .reaction_users(ctx, reaction, None, None)
//         .await?
//         .into_iter()
//         .filter(|user| user.id != kingfisher_id)
//         .collect_vec())
// }

// // Handle a reaction
// pub async fn handle_yeeting(
//     ctx: &serenity::Context,
//     data: &AppState,
//     message: &serenity::Message,
// ) -> Result<()> {
//     let message_id = message.id;

//     // check if message is in the yeet map
//     let yeet_data = match YEET_MAP.get(&message_id) {
//         Some(data) => data.clone(),
//         None => return Ok(()),
//     };

//     let mut did_yay = 0;
//     let mut did_nay = 0;

//     for reaction in &message.reactions {
//         if let serenity::ReactionType::Unicode(emoji) = &reaction.reaction_type {
//             let char = emoji.chars().next().unwrap_or(' ');

//             if char == YEET_YES_REACTION {
//                 did_yay += reaction.count;
//             } else if char == YEET_NO_REACTION {
//                 did_nay += reaction.count;
//             }
//         }
//     }

//     let did_yay = did_yay >= YEET_REQUIRED_REACTION_COUNT;
//     let did_nay = did_nay >= YEET_REQUIRED_REACTION_COUNT;

//     if !did_yay && !did_nay {
//         return Ok(());
//     }

//     // Make sure we don't count too many times
//     if YEET_MAP.remove(&message_id).is_none() {
//         return Ok(());
//     }

//     // This are costly api calls.
//     let yay = get_unique_non_kingfisher_voters(ctx, message, YEET_YES_REACTION).await?;
//     let nay = get_unique_non_kingfisher_voters(ctx, message, YEET_NO_REACTION).await?;

//     // Delete the voting message
//     message.delete(ctx).await.ok(); // Don't care if it succeeds

//     let (target, shooters) = if did_yay {
//         (&yeet_data.victim, yay)
//     } else {
//         (&yeet_data.yeeter, nay)
//     };

//     let time = std::time::Duration::from_secs(YEET_DURATION_SECONDS);
//     let timeout_end = chrono::Utc::now() + time;

//     save_to_yeet_leaderboard(ctx, data, target).await.ok();

//     if yeet_data
//         .guild_id
//         .edit_member(
//             ctx,
//             target,
//             serenity::EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
//         )
//         .await
//         .is_err()
//     {
//         yeet_data
//             .channel_id
//             .send_message(
//                 ctx,
//                 CreateMessage::new().content(
//                     MessageBuilder::new()
//                         .push(format!(
//                             "Sorry {}, but I couldn't yeet {}. Shame them publicly instead.",
//                             shooters.mention_all(),
//                             target.mention()
//                         ))
//                         .build(),
//                 ),
//             )
//             .await?;

//         return Ok(());
//     };

//     let mut message_handle = yeet_data
//         .channel_id
//         .send_message(
//             ctx,
//             CreateMessage::new().content(
//                 MessageBuilder::new()
//                     .push(format!(
//                         "User {} has been yeeted! They will return {}\nBrought to you by: {}",
//                         target.mention(),
//                         timeout_end.discord_relative_timestamp(),
//                         shooters.mention_all(),
//                     ))
//                     .build(),
//             ),
//         )
//         .await?;

//     tokio::time::sleep(time - std::time::Duration::from_secs(1)).await;

//     message_handle
//         .edit(
//             ctx,
//             EditMessage::new().content(format!(
//                 "User {} was yeeted\nBrought to you by: {}",
//                 target.mention(),
//                 shooters.mention_all()
//             )),
//         )
//         .await?;

//     Ok(())
// }

// trait MentionableExt {
//     fn mention_all(&self) -> String;
// }

// impl MentionableExt for Vec<User> {
//     fn mention_all(&self) -> String {
//         self.iter().map(|user| user.mention().to_string()).join(" ")
//     }
// }

// async fn save_to_yeet_leaderboard(
//     ctx: &serenity::Context,
//     data: &AppState,
//     target: &UserId,
// ) -> Result<()> {
//     let target = target.to_user(ctx).await?.id;
//     let yeet_leaderboard = get_yeet_leaderboard(&data.db)?;

//     yeet_leaderboard.increment(target)?;

//     Ok(())
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// struct YeetEntry {
//     user_id: serenity::UserId,
//     count: u64,
// }

// impl PartialOrd for YeetEntry {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         Some(self.cmp(other))
//     }
// }

// impl Ord for YeetEntry {
//     fn cmp(&self, other: &Self) -> std::cmp::Ordering {
//         self.count.cmp(&other.count)
//     }
// }

// /// See who has been yeeted the most
// #[poise::command(slash_command, rename = "yeet_leaderboard", ephemeral = true)]
// pub async fn yeet_leaderboard(ctx: PoiseContext<'_>) -> Result<()> {
//     let mut message_text = String::from("### Yeet leaderboard:\n");
//     let mut yeeted = BinaryHeap::new();

//     let yeet_leaderboard = get_yeet_leaderboard(&ctx.data().db)?;

//     for (user_id, count) in yeet_leaderboard.iter() {
//         yeeted.push(YeetEntry { user_id, count });
//     }

//     for entry in yeeted {
//         message_text.push_str(&format!("{}: {}\n", entry.user_id.mention(), entry.count));
//     }

//     ctx.say(message_text).await?;

//     Ok(())
// }

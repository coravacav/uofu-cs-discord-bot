use crate::data::{PoiseContext, with_db};
use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::America::Denver;
use color_eyre::eyre::{OptionExt, Result};
use poise::serenity_prelude::{
    self as serenity, ButtonStyle, CreateActionRow, CreateButton, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateMessage, EditMember, GuildId, User, UserId,
};
use rusqlite::{OptionalExtension, params};
use std::time::Duration;

#[derive(Debug)]
struct MessageLimit {
    daily_limit: u64,
    imposed_by: Option<u64>,
}

#[derive(Debug)]
struct GuildLimitEntry {
    guild_id: u64,
}

#[derive(Debug)]
struct GuildMessageLimit {
    guild_id: u64,
    daily_limit: u64,
    imposed_by: Option<u64>,
}

/// Returns the current date in Mountain Time as "YYYY-MM-DD"
fn get_current_mt_date() -> String {
    let now_mt = Utc::now().with_timezone(&Denver);
    now_mt.format("%Y-%m-%d").to_string()
}

/// Returns the next midnight in Mountain Time as a UTC DateTime
fn get_next_midnight_mt() -> DateTime<Utc> {
    let now_mt = Utc::now().with_timezone(&Denver);
    let tomorrow = now_mt.date_naive() + chrono::Duration::days(1);
    let midnight_mt = tomorrow.and_hms_opt(0, 0, 0).unwrap();
    Denver
        .from_local_datetime(&midnight_mt)
        .unwrap()
        .with_timezone(&Utc)
}

/// Query user's message limit from database
async fn query_user_limit(user_id: UserId, guild_id: GuildId) -> Result<Option<MessageLimit>> {
    let (user_id, guild_id) = (u64::from(user_id), u64::from(guild_id));
    with_db(move |conn| {
        Ok(conn
            .query_row(
                "SELECT daily_limit, imposed_by FROM message_limit WHERE user_id = ?1 AND guild_id = ?2",
                [user_id, guild_id],
                |row| {
                    Ok(MessageLimit {
                        daily_limit: row.get(0)?,
                        imposed_by: row.get(1)?,
                    })
                },
            )
            .optional()?)
    })
    .await
}

/// Query user's message count from database
async fn query_user_count(user_id: UserId, guild_id: GuildId, date: &str) -> Result<u64> {
    let (user_id, guild_id, date) = (u64::from(user_id), u64::from(guild_id), date.to_owned());
    with_db(move |conn| {
        Ok(conn
            .query_row(
                "SELECT count FROM message_count WHERE user_id = ?1 AND guild_id = ?2 AND reset_date = ?3",
                params![user_id, guild_id, date],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0))
    })
    .await
}

/// Increment message count for user on given date, returns new count
async fn increment_message_count(user_id: UserId, guild_id: GuildId, date: String) -> Result<u64> {
    let (user_id, guild_id) = (u64::from(user_id), u64::from(guild_id));
    with_db(move |conn| {
        // A row from an earlier day restarts at 1.
        Ok(conn.query_row(
            "INSERT INTO message_count (user_id, guild_id, count, reset_date) VALUES (?1, ?2, 1, ?3) \
             ON CONFLICT (user_id, guild_id) DO UPDATE SET \
                 count = CASE WHEN reset_date = excluded.reset_date THEN count + 1 ELSE 1 END, \
                 reset_date = excluded.reset_date \
             RETURNING count",
            params![user_id, guild_id, date],
            |row| row.get(0),
        )?)
    })
    .await
}

/// Create or replace a user's limit and restart today's count at zero
async fn replace_limit(
    user_id: UserId,
    guild_id: GuildId,
    daily_limit: u64,
    imposed_by: Option<UserId>,
) -> Result<()> {
    let (user_id, guild_id) = (u64::from(user_id), u64::from(guild_id));
    let imposed_by = imposed_by.map(u64::from);
    let date = get_current_mt_date();
    with_db(move |conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO message_limit (user_id, guild_id, daily_limit, imposed_by) \
             VALUES (?1, ?2, ?3, ?4)",
            params![user_id, guild_id, daily_limit, imposed_by],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO message_count (user_id, guild_id, count, reset_date) \
             VALUES (?1, ?2, 0, ?3)",
            params![user_id, guild_id, date],
        )?;
        tx.commit()?;
        Ok(())
    })
    .await
}

/// Delete a user's limit in a guild, optionally only if it is self-imposed
async fn delete_limit(user_id: UserId, guild_id: GuildId, only_self_imposed: bool) -> Result<()> {
    let (user_id, guild_id) = (u64::from(user_id), u64::from(guild_id));
    with_db(move |conn| {
        conn.execute(
            "DELETE FROM message_limit WHERE user_id = ?1 AND guild_id = ?2 \
             AND (NOT ?3 OR imposed_by IS NULL)",
            params![user_id, guild_id, only_self_imposed],
        )?;
        Ok(())
    })
    .await
}

/// Apply timeout and send DM notification to user
async fn apply_timeout_and_notify(
    ctx: &serenity::Context,
    message: &serenity::Message,
    user_id: UserId,
    imposed_by: Option<UserId>,
    daily_limit: u64,
) -> Result<()> {
    let timeout_end = get_next_midnight_mt();

    // Apply timeout only if in a guild
    if let Some(guild_id) = message.guild_id {
        let result = guild_id
            .edit_member(
                ctx,
                user_id,
                EditMember::new().disable_communication_until(timeout_end.to_rfc3339()),
            )
            .await;

        if let Err(e) = result {
            tracing::warn!("Failed to timeout user {}: {}", user_id, e);
        }
    }

    // Send DM notification
    let user_result = user_id.to_user(ctx).await;
    if let Ok(user) = user_result {
        let dm_channel_result = user.create_dm_channel(ctx).await;
        if let Ok(dm_channel) = dm_channel_result {
            let message_content = if let Some(mod_id) = imposed_by {
                format!(
                    "⏰ **Message Limit Reached**\n\n\
                     You've hit your daily message limit of **{}** messages.\n\
                     This limit was imposed by a moderator (<@{}>).\n\n\
                     You've been timed out until **midnight Mountain Time** (<t:{}:R>).\n\n\
                     To have this limit removed, please contact a moderator.",
                    daily_limit,
                    mod_id,
                    timeout_end.timestamp()
                )
            } else {
                format!(
                    "⏰ **Message Limit Reached**\n\n\
                     You've hit your self-imposed daily message limit of **{}** messages.\n\n\
                     You've been timed out until **midnight Mountain Time** (<t:{}:R>).\n\n\
                     **To opt out:** Use `/message_limit clear` to remove your limit.\n\
                     **To view progress:** Use `/message_limit view` anytime.",
                    daily_limit,
                    timeout_end.timestamp()
                )
            };

            let send_result = dm_channel
                .send_message(ctx, CreateMessage::new().content(message_content))
                .await;

            if let Err(e) = send_result {
                tracing::warn!("Failed to send DM to user {}: {}", user_id, e);
            }
        } else {
            tracing::warn!("Failed to create DM channel for user {}", user_id);
        }
    } else {
        tracing::warn!("Failed to fetch user {} for DM notification", user_id);
    }

    Ok(())
}

/// Track a message for limit enforcement
pub async fn track_message_for_limit(
    ctx: &serenity::Context,
    message: &serenity::Message,
) -> Result<()> {
    // 1. Filter bots
    if message.author.bot {
        return Ok(());
    }

    // 2. Only track in guilds
    let Some(guild_id) = message.guild_id else {
        return Ok(());
    };

    // 3. Filter commands (messages starting with /)
    if message.content.trim_start().starts_with('/') {
        return Ok(());
    }

    // 4. Query user's limit from database
    let user_id = message.author.id;
    let Some(limit_record) = query_user_limit(user_id, guild_id).await? else {
        return Ok(()); // No limit set
    };

    // 5. Get current Mountain Time date
    let mt_date = get_current_mt_date();

    // 6. Increment message count for today
    let current_count = increment_message_count(user_id, guild_id, mt_date).await?;

    // 7. Check if limit exceeded
    if current_count > limit_record.daily_limit {
        apply_timeout_and_notify(
            ctx,
            message,
            user_id,
            limit_record.imposed_by.map(UserId::new),
            limit_record.daily_limit,
        )
        .await?;
    }

    Ok(())
}

/// Generate a progress bar
fn generate_progress_bar(current: u64, max: u64) -> String {
    let percentage = if max > 0 {
        (current as f64 / max as f64 * 100.0).min(100.0)
    } else {
        0.0
    };

    let filled = (percentage / 10.0).round() as usize;
    let empty = 10 - filled;

    format!(
        "[{}{}] {:.0}%",
        "█".repeat(filled),
        "░".repeat(empty),
        percentage
    )
}

/// Parent command for message limit subcommands
#[poise::command(
    slash_command,
    subcommands("impose", "set", "view", "clear", "remove"),
    rename = "message_limit"
)]
pub async fn message_limit(_ctx: PoiseContext<'_>) -> Result<()> {
    Ok(())
}

/// Impose a message limit on a user (moderator only)
#[poise::command(
    slash_command,
    required_permissions = "MODERATE_MEMBERS",
    ephemeral = true,
    guild_only
)]
pub async fn impose(
    ctx: PoiseContext<'_>,
    #[description = "The user to impose a limit on"] user: User,
    #[description = "Daily message limit (must be positive)"] limit: u64,
) -> Result<()> {
    let user_id = user.id;
    let moderator_id = ctx.author().id;
    let guild_id = ctx.guild_id().ok_or_eyre("Must be used in a guild")?;

    replace_limit(user_id, guild_id, limit, Some(moderator_id)).await?;

    let components = vec![CreateActionRow::Buttons(vec![
        CreateButton::new(format!("view_limit_{}", user_id))
            .label("View Progress")
            .style(ButtonStyle::Primary),
    ])];

    let reply = ctx
        .send(
            poise::CreateReply::default()
                .content(format!(
                    "✅ Set message limit of **{}** messages/day for {}.",
                    limit, user.name
                ))
                .components(components)
                .ephemeral(true),
        )
        .await?;

    let msg = reply.into_message().await?;
    handle_guild_buttons_with_timeout(ctx, &msg, user_id).await?;

    Ok(())
}

/// Set your own message limit
#[poise::command(slash_command, ephemeral = true, guild_only)]
pub async fn set(
    ctx: PoiseContext<'_>,
    #[description = "Daily message limit"] limit: u64,
) -> Result<()> {
    let user_id = ctx.author().id;
    let guild_id = ctx.guild_id().ok_or_eyre("Must be used in a guild")?;

    // Check if a mod-imposed limit exists
    if let Some(existing) = query_user_limit(user_id, guild_id).await?
        && existing.imposed_by.is_some()
    {
        ctx.say(
            "❌ You cannot modify a moderator-imposed limit. Contact a moderator to remove it.",
        )
        .await?;
        return Ok(());
    }

    replace_limit(user_id, guild_id, limit, None).await?;

    let components = vec![CreateActionRow::Buttons(vec![
        CreateButton::new(format!("view_limit_{}", user_id))
            .label("View Progress")
            .style(ButtonStyle::Primary),
        CreateButton::new(format!("clear_limit_{}", user_id))
            .label("Clear Limit")
            .style(ButtonStyle::Danger),
    ])];

    let reply = ctx
        .send(
            poise::CreateReply::default()
                .content(format!(
                    "✅ Set your daily message limit to **{}** messages.",
                    limit
                ))
                .components(components)
                .ephemeral(true),
        )
        .await?;

    let msg = reply.into_message().await?;
    handle_guild_buttons_with_timeout(ctx, &msg, user_id).await?;

    Ok(())
}

/// View message limit and progress (yours or another user's)
#[poise::command(slash_command, ephemeral = true)]
pub async fn view(
    ctx: PoiseContext<'_>,
    #[description = "The user to view (defaults to yourself)"] user: Option<User>,
) -> Result<()> {
    if let Some(guild_id) = ctx.guild_id() {
        // Guild context: show this guild's limit only
        let target = user.as_ref().unwrap_or_else(|| ctx.author());
        let target_id = target.id;

        let Some(limit_record) = query_user_limit(target_id, guild_id).await? else {
            let msg = if target_id == ctx.author().id {
                "ℹ️ No message limit set. Use `/message_limit set` to set one.".to_string()
            } else {
                format!("ℹ️ {} has no message limit set.", target.name)
            };
            ctx.say(msg).await?;
            return Ok(());
        };

        let mt_date = get_current_mt_date();
        let current_count = query_user_count(target_id, guild_id, &mt_date).await?;

        let is_self = target_id == ctx.author().id;
        let (content, components) =
            build_view_response(target_id, &limit_record, current_count, is_self);

        let reply = ctx
            .send(
                poise::CreateReply::default()
                    .content(content)
                    .components(components)
                    .ephemeral(true),
            )
            .await?;

        let msg = reply.into_message().await?;
        handle_guild_buttons_with_timeout(ctx, &msg, target_id).await?;
    } else {
        // DM context: show all limits across all servers
        if user.as_ref().is_some_and(|u| u.id != ctx.author().id) {
            ctx.say("ℹ️ Viewing another user's limits is only available in a server.")
                .await?;
            return Ok(());
        }

        let user_id = ctx.author().id;
        let uid = u64::from(user_id);
        let limits = with_db(move |conn| {
            let limits = conn
                .prepare(
                    "SELECT guild_id, daily_limit, imposed_by FROM message_limit WHERE user_id = ?1",
                )?
                .query_map([uid], |row| {
                    Ok(GuildMessageLimit {
                        guild_id: row.get(0)?,
                        daily_limit: row.get(1)?,
                        imposed_by: row.get(2)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(limits)
        })
        .await?;

        if limits.is_empty() {
            ctx.say("ℹ️ No message limits set. Use `/message_limit set` in a server to set one.")
                .await?;
            return Ok(());
        }

        let mt_date = get_current_mt_date();
        let next_midnight = get_next_midnight_mt();
        let now = Utc::now();
        let duration = next_midnight - now;
        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;

        let mut content = String::from("📊 **Message Limit Status**\n");

        for limit in &limits {
            let gid = GuildId::new(limit.guild_id);
            let guild_name = gid
                .to_partial_guild(ctx.serenity_context())
                .await
                .map(|g| g.name)
                .unwrap_or_else(|_| format!("Server {}", limit.guild_id));

            let count = query_user_count(user_id, gid, &mt_date).await?;
            let progress = generate_progress_bar(count, limit.daily_limit);
            let imposed = if let Some(mod_id) = limit.imposed_by {
                format!("<@{}>", mod_id)
            } else {
                "Self".to_string()
            };

            content.push_str(&format!(
                "\n**{}**\n{} / {} messages | {}\nImposed by: {}\n",
                guild_name, count, limit.daily_limit, progress, imposed
            ));
        }

        content.push_str(&format!(
            "\n**Time until reset:** {}h {}m (midnight MT)",
            hours, minutes
        ));

        ctx.send(
            poise::CreateReply::default()
                .content(content)
                .ephemeral(true),
        )
        .await?;
    }

    Ok(())
}

/// Query all guilds where a user has a self-imposed limit
async fn query_self_imposed_guilds(user_id: UserId) -> Result<Vec<GuildLimitEntry>> {
    let user_id = u64::from(user_id);
    with_db(move |conn| {
        let entries = conn
            .prepare(
                "SELECT guild_id FROM message_limit WHERE user_id = ?1 AND imposed_by IS NULL",
            )?
            .query_map([user_id], |row| {
                Ok(GuildLimitEntry {
                    guild_id: row.get(0)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(entries)
    })
    .await
}

/// Build buttons for selecting which guild to clear a limit from (used in DMs)
async fn build_dm_clear_buttons(
    ctx: &serenity::Context,
    guild_entries: &[GuildLimitEntry],
) -> Vec<CreateActionRow> {
    let mut rows = Vec::new();
    let mut current_row = Vec::new();

    for entry in guild_entries {
        let gid = GuildId::new(entry.guild_id);
        let guild_name = gid
            .to_partial_guild(ctx)
            .await
            .map(|g| g.name)
            .unwrap_or_else(|_| format!("Server {}", entry.guild_id));

        current_row.push(
            CreateButton::new(format!("dm_clear_limit_{}", entry.guild_id))
                .label(guild_name)
                .style(ButtonStyle::Danger),
        );

        if current_row.len() == 5 {
            rows.push(CreateActionRow::Buttons(std::mem::take(&mut current_row)));
        }
    }

    if !current_row.is_empty() {
        rows.push(CreateActionRow::Buttons(current_row));
    }

    rows
}

/// Handle component interactions on a guild message with a 60-second inactivity timeout.
/// Processes view/refresh/clear button clicks inline. Stops listening after timeout.
async fn handle_guild_buttons_with_timeout(
    ctx: PoiseContext<'_>,
    msg: &serenity::Message,
    target_user_id: UserId,
) -> Result<()> {
    let serenity_ctx = ctx.serenity_context();
    let guild_id = ctx.guild_id().ok_or_eyre("Must be used in a guild")?;

    loop {
        let interaction = msg
            .await_component_interaction(serenity_ctx)
            .timeout(Duration::from_secs(60))
            .await;

        let Some(interaction) = interaction else {
            break;
        };

        let custom_id = &interaction.data.custom_id;

        if custom_id.starts_with("view_limit_") || custom_id.starts_with("refresh_limit_") {
            let Some(limit_record) = query_user_limit(target_user_id, guild_id).await? else {
                interaction
                    .create_response(
                        serenity_ctx,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .content("ℹ️ No message limit set.")
                                .components(vec![]),
                        ),
                    )
                    .await?;
                break;
            };

            let mt_date = get_current_mt_date();
            let current_count = query_user_count(target_user_id, guild_id, &mt_date).await?;

            let is_self = interaction.user.id == target_user_id;
            let (content, components) =
                build_view_response(target_user_id, &limit_record, current_count, is_self);

            interaction
                .create_response(
                    serenity_ctx,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content(content)
                            .components(components),
                    ),
                )
                .await?;
        } else if custom_id.starts_with("clear_limit_") {
            let clicker = interaction.user.id;
            if clicker != target_user_id {
                interaction
                    .create_response(
                        serenity_ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("❌ You can only clear your own limit.")
                                .ephemeral(true),
                        ),
                    )
                    .await?;
                continue;
            }

            let Some(limit_record) = query_user_limit(target_user_id, guild_id).await? else {
                interaction
                    .create_response(
                        serenity_ctx,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .content("ℹ️ No message limit is currently set.")
                                .components(vec![]),
                        ),
                    )
                    .await?;
                break;
            };

            if limit_record.imposed_by.is_some() {
                interaction
                    .create_response(
                        serenity_ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(
                                    "❌ Only moderators can remove this limit. Contact a moderator.",
                                )
                                .ephemeral(true),
                        ),
                    )
                    .await?;
                continue;
            }

            delete_limit(target_user_id, guild_id, false).await?;

            interaction
                .create_response(
                    serenity_ctx,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content("✅ Your message limit has been cleared.")
                            .components(vec![]),
                    ),
                )
                .await?;
            break;
        }
    }

    Ok(())
}

/// Clear your self-imposed message limit
#[poise::command(slash_command, ephemeral = true)]
pub async fn clear(ctx: PoiseContext<'_>) -> Result<()> {
    let user_id = ctx.author().id;

    if let Some(guild_id) = ctx.guild_id() {
        // Guild context: clear the single guild-specific limit
        let Some(limit_record) = query_user_limit(user_id, guild_id).await? else {
            ctx.say("ℹ️ No message limit is currently set.").await?;
            return Ok(());
        };

        if limit_record.imposed_by.is_some() {
            ctx.say("❌ Only moderators can remove this limit. Contact a moderator.")
                .await?;
            return Ok(());
        }

        delete_limit(user_id, guild_id, false).await?;

        ctx.say("✅ Your message limit has been cleared.").await?;
    } else {
        // DM context: show buttons to pick which server to clear from
        let guild_entries = query_self_imposed_guilds(user_id).await?;

        if guild_entries.is_empty() {
            ctx.say("ℹ️ You have no self-imposed message limits set in any server.")
                .await?;
            return Ok(());
        }

        let components = build_dm_clear_buttons(ctx.serenity_context(), &guild_entries).await;

        let reply = ctx
            .send(
                poise::CreateReply::default()
                    .content("Select a server to clear your message limit from:")
                    .components(components),
            )
            .await?;

        let msg = reply.into_message().await?;

        loop {
            let interaction = msg
                .await_component_interaction(ctx.serenity_context())
                .timeout(Duration::from_secs(60))
                .await;

            let Some(interaction) = interaction else {
                // Timeout — delete the message
                msg.delete(ctx.serenity_context()).await.ok();
                break;
            };

            // Parse guild ID from the button custom_id
            let Some(guild_id_str) = interaction.data.custom_id.strip_prefix("dm_clear_limit_")
            else {
                continue;
            };

            let Ok(gid) = guild_id_str.parse::<u64>() else {
                continue;
            };
            let guild_id = GuildId::new(gid);

            // Delete the specific limit (only self-imposed)
            delete_limit(user_id, guild_id, true).await?;

            let guild_name = guild_id
                .to_partial_guild(ctx.serenity_context())
                .await
                .map(|g| g.name)
                .unwrap_or_else(|_| format!("Server {}", gid));

            // Re-query remaining limits to rebuild buttons
            let remaining = query_self_imposed_guilds(user_id).await?;

            if remaining.is_empty() {
                interaction
                    .create_response(
                        ctx.serenity_context(),
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .content(format!(
                                    "✅ Cleared your message limit for **{}**. You have no more self-imposed limits.",
                                    guild_name
                                ))
                                .components(vec![]),
                        ),
                    )
                    .await?;

                // Brief pause then delete
                tokio::time::sleep(Duration::from_secs(5)).await;
                msg.delete(ctx.serenity_context()).await.ok();
                break;
            }

            let components = build_dm_clear_buttons(ctx.serenity_context(), &remaining).await;

            interaction
                .create_response(
                    ctx.serenity_context(),
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content(format!(
                                "✅ Cleared your message limit for **{}**.\n\nSelect another server to clear:",
                                guild_name
                            ))
                            .components(components),
                    ),
                )
                .await?;
        }
    }

    Ok(())
}

/// Remove a message limit from a user (moderator only)
#[poise::command(
    slash_command,
    required_permissions = "MODERATE_MEMBERS",
    ephemeral = true,
    guild_only
)]
pub async fn remove(
    ctx: PoiseContext<'_>,
    #[description = "The user to remove the limit from"] user: User,
) -> Result<()> {
    let user_id = user.id;
    let guild_id = ctx.guild_id().ok_or_eyre("Must be used in a guild")?;

    // Check if limit exists
    let limit_exists = query_user_limit(user_id, guild_id).await?.is_some();

    if !limit_exists {
        ctx.say(format!("ℹ️ {} has no message limit set.", user.name))
            .await?;
        return Ok(());
    }

    // Delete both records
    let (uid, gid) = (u64::from(user_id), u64::from(guild_id));
    with_db(move |conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM message_limit WHERE user_id = ?1 AND guild_id = ?2",
            [uid, gid],
        )?;
        tx.execute(
            "DELETE FROM message_count WHERE user_id = ?1 AND guild_id = ?2",
            [uid, gid],
        )?;
        tx.commit()?;
        Ok(())
    })
    .await?;

    ctx.say(format!("✅ Removed message limit for {}.", user.name))
        .await?;

    Ok(())
}

/// Build the view/refresh response content and buttons for a user's limit
fn build_view_response(
    target_user_id: UserId,
    limit_record: &MessageLimit,
    current_count: u64,
    is_self: bool,
) -> (String, Vec<CreateActionRow>) {
    let next_midnight = get_next_midnight_mt();
    let now = Utc::now();
    let duration_until_reset = next_midnight - now;
    let hours_until_reset = duration_until_reset.num_hours();
    let minutes_until_reset = duration_until_reset.num_minutes() % 60;

    let progress_bar = generate_progress_bar(current_count, limit_record.daily_limit);

    let imposed_text = if let Some(mod_id) = limit_record.imposed_by {
        format!("\n**Imposed by:** <@{}>", mod_id)
    } else {
        "\n**Imposed by:** Self".to_string()
    };

    let user_label = if is_self {
        String::new()
    } else {
        format!(" for <@{}>", target_user_id)
    };

    let content = format!(
        "📊 **Message Limit Status**{}\n\n\
         **Progress:** {} / {} messages\n\
         {}\n\
         **Time until reset:** {}h {}m (midnight MT){}\n",
        user_label,
        current_count,
        limit_record.daily_limit,
        progress_bar,
        hours_until_reset,
        minutes_until_reset,
        imposed_text
    );

    let mut buttons = vec![
        CreateButton::new(format!("refresh_limit_{}", target_user_id))
            .label("Refresh")
            .style(ButtonStyle::Secondary),
    ];

    if limit_record.imposed_by.is_none() && is_self {
        buttons.push(
            CreateButton::new(format!("clear_limit_{}", target_user_id))
                .label("Clear Limit")
                .style(ButtonStyle::Danger),
        );
    }

    let components = vec![CreateActionRow::Buttons(buttons)];

    (content, components)
}

#[cfg(test)]
#[tokio::test]
async fn test_limit_persistence() {
    crate::data::setup_db();

    let user = UserId::new(92_001);
    let moderator = UserId::new(92_002);
    let guild = GuildId::new(92_003);
    let other_guild = GuildId::new(92_004);

    assert!(query_user_limit(user, guild).await.unwrap().is_none());
    assert!(replace_limit(user, guild, 0, None).await.is_err());

    replace_limit(user, guild, 5, None).await.unwrap();
    replace_limit(user, other_guild, 7, Some(moderator))
        .await
        .unwrap();
    let limit = query_user_limit(user, guild).await.unwrap().unwrap();
    assert_eq!((limit.daily_limit, limit.imposed_by), (5, None));

    let self_imposed = query_self_imposed_guilds(user).await.unwrap();
    assert_eq!(self_imposed.len(), 1);
    assert_eq!(self_imposed[0].guild_id, u64::from(guild));

    let today = get_current_mt_date();
    assert_eq!(query_user_count(user, guild, &today).await.unwrap(), 0);
    for expected in 1..=3 {
        assert_eq!(
            increment_message_count(user, guild, today.clone())
                .await
                .unwrap(),
            expected
        );
    }
    assert_eq!(
        query_user_count(user, guild, "2000-01-01").await.unwrap(),
        0
    );
    assert_eq!(
        increment_message_count(user, guild, "2999-01-01".to_owned())
            .await
            .unwrap(),
        1,
        "a new day restarts the count"
    );

    delete_limit(user, other_guild, true).await.unwrap();
    assert!(
        query_user_limit(user, other_guild).await.unwrap().is_some(),
        "a self-imposed delete must keep a moderator's limit"
    );
    delete_limit(user, other_guild, false).await.unwrap();
    assert!(query_user_limit(user, other_guild).await.unwrap().is_none());
}

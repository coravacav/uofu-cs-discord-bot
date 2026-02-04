use crate::data::{DB, PoiseContext};
use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::America::Denver;
use color_eyre::eyre::{OptionExt, Result};
use poise::serenity_prelude::{
    self as serenity, ButtonStyle, ComponentInteraction, CreateActionRow, CreateButton,
    CreateInteractionResponse, CreateInteractionResponseMessage, CreateMessage, EditMember,
    GuildId, User, UserId,
};

#[derive(Debug, serde::Deserialize)]
struct MessageLimit {
    daily_limit: u64,
    imposed_by: Option<UserId>,
}

#[derive(Debug, serde::Deserialize)]
struct MessageCount {
    count: u64,
    reset_date: String,
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
    let result: Option<MessageLimit> = DB
        .query("SELECT daily_limit, imposed_by FROM message_limit WHERE user_id = $uid AND guild_id = $gid LIMIT 1")
        .bind(("uid", u64::from(user_id)))
        .bind(("gid", u64::from(guild_id)))
        .await?
        .take(0)?;

    Ok(result)
}

/// Query user's message count from database
async fn query_user_count(user_id: UserId, guild_id: GuildId, date: &str) -> Result<u64> {
    let result: Option<MessageCount> = DB
        .query("SELECT count, reset_date FROM message_count WHERE user_id = $uid AND guild_id = $gid LIMIT 1")
        .bind(("uid", u64::from(user_id)))
        .bind(("gid", u64::from(guild_id)))
        .await?
        .take(0)?;

    Ok(result
        .filter(|r| r.reset_date == date)
        .map(|r| r.count)
        .unwrap_or(0))
}

/// Increment message count for user on given date, returns new count
async fn increment_message_count(user_id: UserId, guild_id: GuildId, date: String) -> Result<u64> {
    let result: Option<MessageCount> = DB
        .query("SELECT count, reset_date FROM message_count WHERE user_id = $uid AND guild_id = $gid LIMIT 1")
        .bind(("uid", u64::from(user_id)))
        .bind(("gid", u64::from(guild_id)))
        .await?
        .take(0)?;

    let new_count = if let Some(record) = result {
        if record.reset_date != date {
            // New day - reset count
            DB.query("UPDATE message_count SET count = 1, reset_date = $date WHERE user_id = $uid AND guild_id = $gid")
                .bind(("uid", u64::from(user_id)))
                .bind(("gid", u64::from(guild_id)))
                .bind(("date", date))
                .await?;
            1
        } else {
            // Same day - increment
            let new_count = record.count + 1;
            DB.query("UPDATE message_count SET count = $count WHERE user_id = $uid AND guild_id = $gid")
                .bind(("uid", u64::from(user_id)))
                .bind(("gid", u64::from(guild_id)))
                .bind(("count", new_count))
                .await?;
            new_count
        }
    } else {
        // First message - create record
        DB.query("CREATE message_count SET user_id = $uid, guild_id = $gid, count = 1, reset_date = $date")
            .bind(("uid", u64::from(user_id)))
            .bind(("gid", u64::from(guild_id)))
            .bind(("date", date))
            .await?;
        1
    };

    Ok(new_count)
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
            limit_record.imposed_by,
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
    rename = "message_limit",
    guild_only
)]
pub async fn message_limit(_ctx: PoiseContext<'_>) -> Result<()> {
    Ok(())
}

/// Impose a message limit on a user (moderator only)
#[poise::command(
    slash_command,
    required_permissions = "MODERATE_MEMBERS",
    ephemeral = true
)]
pub async fn impose(
    ctx: PoiseContext<'_>,
    #[description = "The user to impose a limit on"] user: User,
    #[description = "Daily message limit (must be positive)"] limit: u64,
) -> Result<()> {
    let user_id = user.id;
    let moderator_id = ctx.author().id;
    let guild_id = ctx.guild_id().ok_or_eyre("Must be used in a guild")?;

    // Create or update the message limit
    DB.query(
        "DELETE FROM message_limit WHERE user_id = $uid AND guild_id = $gid;
             CREATE message_limit SET user_id = $uid, guild_id = $gid, daily_limit = $limit, imposed_by = $mod_id;",
    )
    .bind(("uid", u64::from(user_id)))
    .bind(("gid", u64::from(guild_id)))
    .bind(("limit", limit))
    .bind(("mod_id", u64::from(moderator_id)))
    .await?;

    // Initialize message count for today
    let mt_date = get_current_mt_date();
    DB.query(
        "DELETE FROM message_count WHERE user_id = $uid AND guild_id = $gid;
             CREATE message_count SET user_id = $uid, guild_id = $gid, count = 0, reset_date = $date;",
    )
    .bind(("uid", u64::from(user_id)))
    .bind(("gid", u64::from(guild_id)))
    .bind(("date", mt_date))
    .await?;

    let components = vec![CreateActionRow::Buttons(vec![
        CreateButton::new(format!("view_limit_{}", user_id))
            .label("View Progress")
            .style(ButtonStyle::Primary),
    ])];

    ctx.send(
        poise::CreateReply::default()
            .content(format!(
                "✅ Set message limit of **{}** messages/day for {}.",
                limit, user.name
            ))
            .components(components)
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Set your own message limit
#[poise::command(slash_command, ephemeral = true)]
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

    // Create or update the message limit
    DB.query(
        "DELETE FROM message_limit WHERE user_id = $uid AND guild_id = $gid;
             CREATE message_limit SET user_id = $uid, guild_id = $gid, daily_limit = $limit;",
    )
    .bind(("uid", u64::from(user_id)))
    .bind(("gid", u64::from(guild_id)))
    .bind(("limit", limit))
    .await?;

    // Initialize message count for today
    let mt_date = get_current_mt_date();
    DB.query(
        "DELETE FROM message_count WHERE user_id = $uid AND guild_id = $gid;
             CREATE message_count SET user_id = $uid, guild_id = $gid, count = 0, reset_date = $date;",
    )
    .bind(("uid", u64::from(user_id)))
    .bind(("gid", u64::from(guild_id)))
    .bind(("date", mt_date))
    .await?;

    let components = vec![CreateActionRow::Buttons(vec![
        CreateButton::new(format!("view_limit_{}", user_id))
            .label("View Progress")
            .style(ButtonStyle::Primary),
        CreateButton::new(format!("clear_limit_{}", user_id))
            .label("Clear Limit")
            .style(ButtonStyle::Danger),
    ])];

    ctx.send(
        poise::CreateReply::default()
            .content(format!(
                "✅ Set your daily message limit to **{}** messages.",
                limit
            ))
            .components(components)
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// View your current message limit and progress
#[poise::command(slash_command, ephemeral = true)]
pub async fn view(ctx: PoiseContext<'_>) -> Result<()> {
    let user_id = ctx.author().id;
    let guild_id = ctx.guild_id().ok_or_eyre("Must be used in a guild")?;

    // Query limit
    let Some(limit_record) = query_user_limit(user_id, guild_id).await? else {
        ctx.say("ℹ️ No message limit set. Use `/message_limit set` to set one.")
            .await?;
        return Ok(());
    };

    // Query count
    let mt_date = get_current_mt_date();
    let current_count = query_user_count(user_id, guild_id, &mt_date).await?;

    let (content, components) = build_view_response(user_id, &limit_record, current_count);

    ctx.send(
        poise::CreateReply::default()
            .content(content)
            .components(components)
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Clear your self-imposed message limit
#[poise::command(slash_command, ephemeral = true)]
pub async fn clear(ctx: PoiseContext<'_>) -> Result<()> {
    let user_id = ctx.author().id;
    let guild_id = ctx.guild_id().ok_or_eyre("Must be used in a guild")?;

    // Check if limit exists
    let Some(limit_record) = query_user_limit(user_id, guild_id).await? else {
        ctx.say("ℹ️ No message limit is currently set.").await?;
        return Ok(());
    };

    // Check if it's mod-imposed
    if limit_record.imposed_by.is_some() {
        ctx.say("❌ Only moderators can remove this limit. Contact a moderator.")
            .await?;
        return Ok(());
    }

    // Delete the limit
    DB.query("DELETE FROM message_limit WHERE user_id = $uid AND guild_id = $gid")
        .bind(("uid", u64::from(user_id)))
        .bind(("gid", u64::from(guild_id)))
        .await?;

    ctx.say("✅ Your message limit has been cleared.").await?;

    Ok(())
}

/// Remove a message limit from a user (moderator only)
#[poise::command(
    slash_command,
    required_permissions = "MODERATE_MEMBERS",
    ephemeral = true
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
    DB.query(
        "DELETE FROM message_limit WHERE user_id = $uid AND guild_id = $gid;
             DELETE FROM message_count WHERE user_id = $uid AND guild_id = $gid;",
    )
    .bind(("uid", u64::from(user_id)))
    .bind(("gid", u64::from(guild_id)))
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
        "".to_string()
    };

    let content = format!(
        "📊 **Message Limit Status**\n\n\
         **Progress:** {} / {} messages\n\
         {}\n\
         **Time until reset:** {}h {}m (midnight MT){}\n",
        current_count,
        limit_record.daily_limit,
        progress_bar,
        hours_until_reset,
        minutes_until_reset,
        imposed_text
    );

    let mut buttons = vec![CreateButton::new(format!("refresh_limit_{}", target_user_id))
        .label("Refresh")
        .style(ButtonStyle::Secondary)];

    if limit_record.imposed_by.is_none() {
        buttons.push(
            CreateButton::new(format!("clear_limit_{}", target_user_id))
                .label("Clear Limit")
                .style(ButtonStyle::Danger),
        );
    }

    let components = vec![CreateActionRow::Buttons(buttons)];

    (content, components)
}

/// Handle button interactions for message limit buttons
pub async fn handle_message_limit_interaction(
    ctx: &serenity::Context,
    interaction: &ComponentInteraction,
) -> Result<bool> {
    let custom_id = &interaction.data.custom_id;

    let (action, target_user_id) =
        if let Some(id_str) = custom_id.strip_prefix("view_limit_")
            .or_else(|| custom_id.strip_prefix("refresh_limit_"))
        {
            let uid: u64 = id_str.parse().map_err(|_| color_eyre::eyre::eyre!("Invalid user ID in button"))?;
            ("view", UserId::new(uid))
        } else if let Some(id_str) = custom_id.strip_prefix("clear_limit_") {
            let uid: u64 = id_str.parse().map_err(|_| color_eyre::eyre::eyre!("Invalid user ID in button"))?;
            ("clear", UserId::new(uid))
        } else {
            return Ok(false);
        };

    let guild_id = interaction
        .guild_id
        .ok_or_eyre("Button must be used in a guild")?;

    match action {
        "view" => {
            let Some(limit_record) = query_user_limit(target_user_id, guild_id).await? else {
                interaction
                    .create_response(
                        ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("ℹ️ No message limit set.")
                                .ephemeral(true),
                        ),
                    )
                    .await?;
                return Ok(true);
            };

            let mt_date = get_current_mt_date();
            let current_count =
                query_user_count(target_user_id, guild_id, &mt_date).await?;

            let (content, components) =
                build_view_response(target_user_id, &limit_record, current_count);

            interaction
                .create_response(
                    ctx,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content(content)
                            .components(components),
                    ),
                )
                .await?;
        }
        "clear" => {
            let clicker = interaction.user.id;
            if clicker != target_user_id {
                interaction
                    .create_response(
                        ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("❌ You can only clear your own limit.")
                                .ephemeral(true),
                        ),
                    )
                    .await?;
                return Ok(true);
            }

            let Some(limit_record) = query_user_limit(target_user_id, guild_id).await? else {
                interaction
                    .create_response(
                        ctx,
                        CreateInteractionResponse::UpdateMessage(
                            CreateInteractionResponseMessage::new()
                                .content("ℹ️ No message limit is currently set.")
                                .components(vec![]),
                        ),
                    )
                    .await?;
                return Ok(true);
            };

            if limit_record.imposed_by.is_some() {
                interaction
                    .create_response(
                        ctx,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(
                                    "❌ Only moderators can remove this limit. Contact a moderator.",
                                )
                                .ephemeral(true),
                        ),
                    )
                    .await?;
                return Ok(true);
            }

            DB.query("DELETE FROM message_limit WHERE user_id = $uid AND guild_id = $gid")
                .bind(("uid", u64::from(target_user_id)))
                .bind(("gid", u64::from(guild_id)))
                .await?;

            interaction
                .create_response(
                    ctx,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content("✅ Your message limit has been cleared.")
                            .components(vec![]),
                    ),
                )
                .await?;
        }
        _ => unreachable!(),
    }

    Ok(true)
}

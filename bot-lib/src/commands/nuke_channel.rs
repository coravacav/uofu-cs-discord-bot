use std::{fs, path::Path, time::Duration};

use color_eyre::eyre::Result;
use poise::{
    CreateReply,
    serenity_prelude::{
        ButtonStyle, ChannelId, ChannelType, CreateActionRow, CreateButton, CreateChannel,
        CreateInteractionResponse, CreateInteractionResponseMessage, GuildChannel,
    },
};

use crate::{
    commands::{ChannelBackup, backup_channel, is_stefan},
    data::PoiseContext,
};

/// Back up a channel (messages + attachments), then replace it with an empty copy
#[poise::command(slash_command, ephemeral = true, guild_only, check = is_stefan)]
pub async fn backup_and_nuke_channel(
    ctx: PoiseContext<'_>,
    #[description = "ID of the channel to back up and nuke"] channel_id: String,
) -> Result<()> {
    let Some(channel_id) = parse_channel_id(&channel_id) else {
        ctx.say(format!("`{channel_id}` is not a valid channel ID"))
            .await?;
        return Ok(());
    };

    let Some(channel) = get_nukeable_channel(ctx, channel_id).await? else {
        return Ok(());
    };

    ctx.defer_ephemeral().await?;
    let backup = run_backup(ctx, &channel).await?;

    confirm_and_nuke(
        ctx,
        &channel,
        format!(
            "{}\nNuke it? It will be replaced with an empty copy.",
            backup_summary(&channel, &backup)
        ),
        "Cancelled, the backup was kept.",
    )
    .await
}

/// Back up the current channel (messages + attachments) without touching it
#[poise::command(slash_command, ephemeral = true, guild_only, check = is_stefan)]
pub async fn backup_current_channel(ctx: PoiseContext<'_>) -> Result<()> {
    let Some(channel) = get_nukeable_channel(ctx, ctx.channel_id()).await? else {
        return Ok(());
    };

    ctx.defer_ephemeral().await?;
    let backup = run_backup(ctx, &channel).await?;

    ctx.say(backup_summary(&channel, &backup)).await?;

    Ok(())
}

/// Replace the current channel with an empty copy, without backing it up first
#[poise::command(slash_command, ephemeral = true, guild_only, check = is_stefan)]
pub async fn reset_current_channel(ctx: PoiseContext<'_>) -> Result<()> {
    let Some(channel) = get_nukeable_channel(ctx, ctx.channel_id()).await? else {
        return Ok(());
    };

    let directory = Path::new("extracts").join(backup_directory_name(&channel));
    let backup_status = if is_complete_backup(&directory) {
        format!("Found a complete backup at `{}`.", directory.display())
    } else {
        format!(
            "⚠️ No complete backup at `{}`, everything in this channel will be lost. \
             Run `/backup_current_channel` first if you need one.",
            directory.display()
        )
    };

    confirm_and_nuke(
        ctx,
        &channel,
        format!(
            "{backup_status}\nReset <#{}> (`{}`)? It will be replaced with an empty copy.",
            channel.id, channel.name
        ),
        "Cancelled.",
    )
    .await
}

/// Fetches the channel, telling the user and returning `None` if it can't be backed up and
/// recreated.
async fn get_nukeable_channel(
    ctx: PoiseContext<'_>,
    channel_id: ChannelId,
) -> Result<Option<GuildChannel>> {
    let Ok(channel) = channel_id.to_channel(&ctx).await else {
        ctx.say(format!("Couldn't find channel `{channel_id}`"))
            .await?;
        return Ok(None);
    };

    let Some(channel) = channel.guild() else {
        ctx.say("That channel is not in a guild").await?;
        return Ok(None);
    };

    if !matches!(
        channel.kind,
        ChannelType::Text | ChannelType::News | ChannelType::Voice | ChannelType::Stage
    ) {
        ctx.say(format!(
            "Can't do that to `{}`, channels of type {:?} can't be backed up and recreated",
            channel.name, channel.kind
        ))
        .await?;
        return Ok(None);
    }

    Ok(Some(channel))
}

/// The ID suffix keeps a later backup of the recreated channel (same name, new ID) from
/// overwriting this one.
fn backup_directory_name(channel: &GuildChannel) -> String {
    format!("{}-{}", channel.name, channel.id)
}

/// `backup_channel` writes the elapsed time as the last line once everything is saved.
fn is_complete_backup(directory: &Path) -> bool {
    fs::read_to_string(directory.join("messages.txt")).is_ok_and(|messages| {
        messages
            .lines()
            .last()
            .is_some_and(|line| line.starts_with("Elapsed time:"))
    })
}

async fn run_backup(ctx: PoiseContext<'_>, channel: &GuildChannel) -> Result<ChannelBackup> {
    backup_channel(
        ctx.serenity_context(),
        channel.id,
        &backup_directory_name(channel),
    )
    .await
}

fn backup_summary(channel: &GuildChannel, backup: &ChannelBackup) -> String {
    format!(
        "Backed up {} messages and {}/{} attachments from <#{}> (`{}`) to `{}`.",
        backup.message_count,
        backup.attachment_count - backup.failed_attachment_count,
        backup.attachment_count,
        channel.id,
        channel.name,
        backup.directory.display(),
    )
}

/// Asks for confirmation with buttons, then replaces the channel with an empty copy.
async fn confirm_and_nuke(
    ctx: PoiseContext<'_>,
    channel: &GuildChannel,
    prompt: String,
    cancelled_status: &str,
) -> Result<()> {
    let reply =
        ctx.send(CreateReply::default().content(prompt).components(vec![
            CreateActionRow::Buttons(vec![
                    CreateButton::new("nuke_channel_confirm")
                        .label("Nuke it")
                        .style(ButtonStyle::Danger),
                    CreateButton::new("nuke_channel_cancel")
                        .label("Cancel")
                        .style(ButtonStyle::Secondary),
                ]),
        ]))
        .await?;

    let interaction = reply
        .message()
        .await?
        .await_component_interaction(ctx.serenity_context())
        .author_id(ctx.author().id)
        .timeout(Duration::from_secs(60))
        .await;

    let confirmed = interaction
        .as_ref()
        .is_some_and(|i| i.data.custom_id == "nuke_channel_confirm");

    let status = if confirmed {
        "Nuking..."
    } else {
        cancelled_status
    };
    match &interaction {
        Some(interaction) => {
            interaction
                .create_response(
                    ctx,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content(status)
                            .components(vec![]),
                    ),
                )
                .await?;
        }
        None => {
            reply
                .edit(
                    ctx,
                    CreateReply::default().content(status).components(vec![]),
                )
                .await?;
        }
    }

    if !confirmed {
        tracing::info!("Nuke of #{} ({}) cancelled", channel.name, channel.id);
        return Ok(());
    }

    let new_channel = recreate_channel(&ctx, channel).await?;
    channel.id.delete(&ctx).await?;
    tracing::info!(
        "Nuked #{} ({}), replaced with {}",
        channel.name,
        channel.id,
        new_channel.id
    );

    // The reply is gone if the command was run inside the nuked channel.
    ctx.say(format!("Nuked! New channel: <#{}>", new_channel.id))
        .await
        .ok();

    Ok(())
}

fn parse_channel_id(input: &str) -> Option<ChannelId> {
    let input = input.trim();
    let input = input
        .strip_prefix("<#")
        .and_then(|s| s.strip_suffix('>'))
        .unwrap_or(input);

    input
        .parse::<u64>()
        .ok()
        .filter(|&id| id != 0)
        .map(ChannelId::new)
}

async fn recreate_channel(ctx: &PoiseContext<'_>, channel: &GuildChannel) -> Result<GuildChannel> {
    let mut builder = CreateChannel::new(channel.name.clone())
        .kind(channel.kind)
        .permissions(channel.permission_overwrites.clone())
        .position(channel.position)
        .nsfw(channel.nsfw);

    if let Some(parent_id) = channel.parent_id {
        builder = builder.category(parent_id);
    }
    if let Some(topic) = &channel.topic {
        builder = builder.topic(topic.clone());
    }
    if let Some(rate_limit) = channel.rate_limit_per_user {
        builder = builder.rate_limit_per_user(rate_limit);
    }
    if let Some(bitrate) = channel.bitrate {
        builder = builder.bitrate(bitrate);
    }
    if let Some(user_limit) = channel.user_limit {
        builder = builder.user_limit(user_limit);
    }
    if let Some(rtc_region) = &channel.rtc_region {
        builder = builder.rtc_region(rtc_region.clone());
    }
    if let Some(video_quality_mode) = channel.video_quality_mode {
        builder = builder.video_quality_mode(video_quality_mode);
    }
    if let Some(duration) = channel.default_auto_archive_duration {
        builder = builder.default_auto_archive_duration(duration);
    }

    Ok(channel.guild_id.create_channel(ctx, builder).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raw_ids_and_mentions() {
        assert_eq!(
            parse_channel_id("1274560000102236282"),
            Some(ChannelId::new(1274560000102236282))
        );
        assert_eq!(
            parse_channel_id(" <#1274560000102236282> "),
            Some(ChannelId::new(1274560000102236282))
        );
        assert_eq!(parse_channel_id("0"), None);
        assert_eq!(parse_channel_id("general"), None);
    }

    #[test]
    fn only_finished_backups_are_complete() {
        let directory =
            std::env::temp_dir().join(format!("nuke-channel-test-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let messages = directory.join("messages.txt");

        assert!(!is_complete_backup(&directory));

        fs::write(&messages, "someone (2024-09-23T22:57:27.817Z)\nhi\n\n").unwrap();
        assert!(!is_complete_backup(&directory));

        fs::write(
            &messages,
            "someone (2024-09-23T22:57:27.817Z)\nhi\n\nElapsed time: 1.5s seconds\n",
        )
        .unwrap();
        assert!(is_complete_backup(&directory));

        fs::remove_dir_all(&directory).unwrap();
    }
}

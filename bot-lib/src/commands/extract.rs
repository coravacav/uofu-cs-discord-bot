use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    pin::pin,
    sync::{Arc, atomic::AtomicUsize},
};

use bot_traits::ForwardRefToTracing;
use color_eyre::eyre::{Result, bail};
use futures::StreamExt;
use poise::serenity_prelude::{ChannelId, Context, CreateMessage, MessageSnapshot};
use tokio::task::JoinSet;

use crate::{
    commands::{get_all_class_general_channels, is_stefan},
    data::PoiseContext,
};

#[poise::command(slash_command, ephemeral = true, guild_only, check = is_stefan)]
pub async fn extract_all_class_channels(ctx: PoiseContext<'_>) -> Result<()> {
    ctx.defer_ephemeral().await?;
    let channels = get_all_class_general_channels(&ctx).unwrap_or_default();

    for channel in &channels {
        let ctx = ctx.serenity_context().clone();

        let completed_count = AtomicUsize::new(0);
        let channel_id = channel.id;
        let channels_len = channels.len();

        tokio::spawn(async move {
            read_chat(ctx, channel_id).await.ok();
            let count = completed_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tracing::info!("Completed {}/{} channels", count, channels_len);
        });
    }

    ctx.reply("Process started!").await?;

    Ok(())
}

#[poise::command(slash_command, ephemeral = true, guild_only, check = is_stefan)]
pub async fn extract_current_channel(ctx: PoiseContext<'_>) -> Result<()> {
    let channel_id = ctx.channel_id();

    ctx.reply("Process started!").await?;

    let ctx = ctx.serenity_context().clone();
    tokio::spawn(async move {
        read_chat(ctx, channel_id).await.ok();
    });

    Ok(())
}

async fn read_chat(ctx: Context, channel_id: ChannelId) -> Result<()> {
    let directory_name = channel_id.name(&ctx).await?;
    backup_channel(&ctx, channel_id, &directory_name).await?;
    Ok(())
}

pub(crate) struct ChannelBackup {
    pub directory: PathBuf,
    pub message_count: usize,
    pub attachment_count: usize,
    pub failed_attachment_count: usize,
}

/// Writes every message in the channel to `extracts/<directory_name>/messages.txt` and
/// downloads all attachments next to it. Only returns once every download has finished.
pub(crate) async fn backup_channel(
    ctx: &Context,
    channel_id: ChannelId,
    directory_name: &str,
) -> Result<ChannelBackup> {
    let start = std::time::Instant::now();

    let base_directory = Path::new("extracts");
    if !base_directory.exists() {
        fs::create_dir(base_directory)?;
    }

    let directory = Arc::new(base_directory.join(directory_name));
    if !directory.exists() {
        fs::create_dir(directory.as_ref())?;
    }

    let message_output_file = fs::File::create(directory.join("messages.txt"))?;
    let mut message_output_file = std::io::BufWriter::new(message_output_file);

    let reqwest = reqwest::Client::new();

    let mut messages = pin!(channel_id.messages_iter(ctx));

    let mut attachments = String::new();
    let mut downloads = JoinSet::new();
    let mut message_count = 0;

    while let Some(message) = messages.next().await {
        let Ok(message) = message else {
            tracing::warn!("Failed to get message {:?}", message);
            continue;
        };

        message_count += 1;
        let mut attachment_count = 0;
        attachments.clear();

        for attachment in message.attachments.into_iter().chain(
            message
                .message_snapshots
                .iter()
                .flat_map(|m| m.attachments.clone()),
        ) {
            attachment_count += 1;
            use std::fmt::Write;
            let filename = format!(
                "{}.{}.{}",
                attachment_count, message.id, attachment.filename
            );
            write!(attachments, "\nAttachment {attachment_count}: {filename}")?;

            let directory = Arc::clone(&directory);
            let url = attachment.url.clone();
            let reqwest = reqwest.clone();
            downloads.spawn(async move { download_file(reqwest, url, filename, &directory).await });
        }

        write!(
            message_output_file,
            "{:<25} ({})",
            message.author.name, message.timestamp
        )?;

        if !message.content.is_empty() {
            write!(message_output_file, "\n{}", message.content)?;
        }

        for MessageSnapshot { content, .. } in &message.message_snapshots {
            if !content.is_empty() {
                write!(message_output_file, "\nforwarded message -> {}", content)?;
            }
        }

        if !message.reactions.is_empty() {
            write!(message_output_file, "\nReactions:")?;
        }
        for reaction in &message.reactions {
            write!(
                message_output_file,
                " ({} - {})",
                reaction.reaction_type, reaction.count
            )?;
        }

        writeln!(message_output_file, "{attachments}\n")?;
    }

    let attachment_count = downloads.len();
    let mut failed_attachment_count = 0;
    while let Some(result) = downloads.join_next().await {
        if result
            .trace_err_ok()
            .and_then(|r| r.trace_err_ok())
            .is_none()
        {
            failed_attachment_count += 1;
        }
    }

    writeln!(
        message_output_file,
        "Elapsed time: {:?} seconds",
        start.elapsed()
    )?;
    message_output_file.flush()?;

    ChannelId::new(1274560000102236282)
        .send_message(
            ctx,
            CreateMessage::new().content(format!("Done archiving {directory_name}")),
        )
        .await?;

    Ok(ChannelBackup {
        directory: directory.as_ref().clone(),
        message_count,
        attachment_count,
        failed_attachment_count,
    })
}

async fn download_file(
    reqwest: reqwest::Client,
    url: String,
    filename: String,
    directory: &Path,
) -> Result<()> {
    let output_file = directory.join(filename);
    if output_file.exists() {
        return Ok(());
    }

    tracing::info!("Downloading attachment {:?}", url);

    let response = reqwest.get(&url).send().await?;
    if !response.status().is_success() {
        bail!("Failed to get attachment {url:?}: {}", response.status());
    }
    let bytes = response.bytes().await?;

    fs::File::create(output_file)?.write_all(&bytes)?;

    Ok(())
}

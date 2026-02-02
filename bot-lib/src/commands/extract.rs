use std::{
    fs,
    io::Write,
    path::Path,
    pin::pin,
    sync::{Arc, atomic::AtomicUsize},
};

use bot_traits::ForwardRefToTracing;
use color_eyre::eyre::Result;
use futures::StreamExt;
use poise::serenity_prelude::{ChannelId, Context, CreateMessage, MessageSnapshot};

use crate::{
    commands::{get_all_class_general_channels, is_stefan},
    data::PoiseContext,
};

#[poise::command(slash_command, ephemeral = true, check = is_stefan)]
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

#[poise::command(slash_command, ephemeral = true, check = is_stefan)]
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
    let start = std::time::Instant::now();

    let base_directory = Path::new("extracts");
    if !base_directory.exists() {
        fs::create_dir(base_directory)?;
    }

    let directory_name = channel_id.name(&ctx).await?;
    let directory = Arc::new(base_directory.join(&directory_name));
    if !directory.exists() {
        fs::create_dir(directory.as_ref())?;
    }

    let message_output_file = fs::File::create(directory.join("messages.txt"))?;
    let mut message_output_file = std::io::BufWriter::new(message_output_file);

    let reqwest = reqwest::Client::new();

    let mut messages = pin!(channel_id.messages_iter(&ctx));

    let mut attachments = String::new();

    while let Some(message) = messages.next().await {
        let Ok(message) = message else {
            tracing::warn!("Failed to get message {:?}", message);
            continue;
        };

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
            tokio::spawn(async move {
                download_file(reqwest, url, filename, &directory)
                    .await
                    .trace_err_ok();
            });
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

    writeln!(
        message_output_file,
        "Elapsed time: {:?} seconds",
        start.elapsed()
    )?;

    ChannelId::new(1274560000102236282)
        .send_message(
            &ctx,
            CreateMessage::new().content(format!("Done archiving {directory_name}")),
        )
        .await?;

    Ok(())
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

    let Ok(bytes) = reqwest.get(&url).send().await else {
        tracing::warn!("Failed to get attachment {:?}", url);
        return Ok(());
    };

    let Ok(bytes) = bytes.bytes().await else {
        tracing::warn!("Failed to get attachment bytes {:?}", url);
        return Ok(());
    };

    let Ok(mut file) = fs::File::create(output_file) else {
        tracing::warn!("Failed to create file {:?}", url);
        return Ok(());
    };

    file.write_all(&bytes).ok();

    Ok(())
}

use std::{fs, io::Write, path::Path, pin::pin, sync::Arc};

use bot_traits::ForwardRefToTracing;
use color_eyre::eyre::Result;
use futures::StreamExt;
use poise::serenity_prelude::{ChannelId, Context, CreateMessage};

use crate::{
    commands::{get_all_class_general_channels, is_stefan},
    data::PoiseContext,
};

#[poise::command(slash_command, ephemeral = true, check = is_stefan)]
pub async fn extract_all_class_channels(ctx: PoiseContext<'_>) -> Result<()> {
    ctx.defer_ephemeral().await?;

    for channel in get_all_class_general_channels(&ctx).unwrap_or_default() {
        let ctx = ctx.serenity_context().clone();

        tokio::spawn(async move {
            read_chat(ctx, channel.id).await.ok();
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

    let directory_name = channel_id.name(&ctx).await?;

    if !Path::new(&directory_name).exists() {
        fs::create_dir(&directory_name)?;
    }

    let message_output_file = fs::File::create(format!("{}/messages.txt", directory_name))?;
    let mut message_output_file = std::io::BufWriter::new(message_output_file);

    let reqwest = reqwest::Client::new();

    let mut messages = pin!(channel_id.messages_iter(&ctx));

    let mut attachments = String::new();
    let directory_name = Arc::new(directory_name);

    while let Some(message) = messages.next().await {
        let Ok(message) = message else {
            tracing::warn!("Failed to get message {:?}", message);
            continue;
        };

        let mut attachment_count = 0;
        attachments.clear();

        for attachment in message.attachments.into_iter()
        // See Cargo.toml
        // .chain(
        //     message
        //         .message_snapshots
        //         .iter()
        //         .flatten()
        //         .flat_map(|m| m.message.attachments.clone()),
        // )
        {
            attachment_count += 1;
            use std::fmt::Write;
            let filename = format!(
                "{}.{}.{}",
                attachment_count, message.id, attachment.filename
            );
            write!(
                attachments,
                "\nAttachment {}: {}",
                attachment_count, filename
            )?;

            let directory_name = Arc::clone(&directory_name);
            let url = attachment.url.clone();
            let reqwest = reqwest.clone();
            tokio::spawn(async move {
                download_file(reqwest, url, filename, &directory_name)
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

        // See Cargo.toml
        // if let Some(message_snapshots) = &message.message_snapshots {
        //     for MessageSnapshotContainer { message, .. } in message_snapshots {
        //         if !message.content.is_empty() {
        //             writeln!(
        //                 message_output_file,
        //                 "\nforwarded message -> {}",
        //                 message.content
        //             )?;
        //         }
        //     }
        // }

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

        writeln!(message_output_file, "{}\n", attachments)?;
    }

    let elapsed = start.elapsed();
    writeln!(
        message_output_file,
        "Elapsed time: {}.{:03} seconds",
        elapsed.as_secs(),
        elapsed.subsec_millis()
    )?;

    ChannelId::new(1274560000102236282)
        .send_message(
            &ctx,
            CreateMessage::new().content(format!("Done archiving {}", directory_name)),
        )
        .await?;

    Ok(())
}

async fn download_file(
    reqwest: reqwest::Client,
    url: String,
    filename: String,
    directory_name: &str,
) -> Result<()> {
    let output_file = format!("{}/{}", directory_name, filename);
    if Path::new(&output_file).exists() {
        return Ok(());
    }

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

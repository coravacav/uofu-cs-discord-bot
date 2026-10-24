use std::time::Duration;

use color_eyre::eyre::Result;
use poise::{
    CreateReply,
    serenity_prelude::{
        ButtonStyle, ChannelId, ChannelType, CreateActionRow, CreateButton, CreateChannel,
        CreateInteractionResponse, CreateInteractionResponseMessage, GuildChannel,
    },
};

use crate::{
    commands::{backup_channel, is_stefan},
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

    let Ok(channel) = channel_id.to_channel(&ctx).await else {
        ctx.say(format!("Couldn't find channel `{channel_id}`"))
            .await?;
        return Ok(());
    };

    let Some(channel) = channel.guild() else {
        ctx.say("That channel is not in a guild").await?;
        return Ok(());
    };

    if !matches!(
        channel.kind,
        ChannelType::Text | ChannelType::News | ChannelType::Voice | ChannelType::Stage
    ) {
        ctx.say(format!(
            "Can't nuke `{}`, channels of type {:?} can't be backed up and recreated",
            channel.name, channel.kind
        ))
        .await?;
        return Ok(());
    }

    ctx.defer_ephemeral().await?;

    // The ID suffix keeps a later nuke of the recreated channel (same name, new ID) from
    // overwriting this backup.
    let directory_name = format!("{}-{}", channel.name, channel.id);
    let backup = backup_channel(ctx.serenity_context(), channel.id, &directory_name).await?;

    let reply = ctx
        .send(
            CreateReply::default()
                .content(format!(
                    "Backed up {} messages and {}/{} attachments from <#{}> (`{}`) to `{}`.\n\
                     Nuke it? It will be replaced with an empty copy.",
                    backup.message_count,
                    backup.attachment_count - backup.failed_attachment_count,
                    backup.attachment_count,
                    channel.id,
                    channel.name,
                    backup.directory.display(),
                ))
                .components(vec![CreateActionRow::Buttons(vec![
                    CreateButton::new("nuke_channel_confirm")
                        .label("Nuke it")
                        .style(ButtonStyle::Danger),
                    CreateButton::new("nuke_channel_cancel")
                        .label("Cancel")
                        .style(ButtonStyle::Secondary),
                ])]),
        )
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
        "Cancelled, the backup was kept."
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
        return Ok(());
    }

    let new_channel = recreate_channel(&ctx, &channel).await?;
    channel.id.delete(&ctx).await?;

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
}

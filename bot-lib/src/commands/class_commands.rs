use crate::{
    commands::{get_member, is_stefan},
    courses::{CourseIdent, get_course},
    data::PoiseContext,
    utils::SendReplyEphemeral,
};
use color_eyre::eyre::{OptionExt, Result, WrapErr};
use itertools::Itertools;
use poise::serenity_prelude::{
    ChannelType, Context, CreateChannel, EditRole, GuildChannel, PermissionOverwrite,
    PermissionOverwriteType, Permissions, Role, RoleId,
};
use regex::Regex;
use rustc_hash::FxHashSet;
use std::{collections::HashMap, fmt::Write, sync::LazyLock, time::Duration};

static CLASS_ROLE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\w+ \d+$").unwrap());

fn get_class_roles(roles: HashMap<RoleId, Role>) -> impl Iterator<Item = Role> {
    roles
        .into_values()
        .filter(|role| CLASS_ROLE_REGEX.is_match(&role.name))
        .sorted_by(|l, r| l.name.cmp(&r.name))
}

/// List all classes you can join
#[poise::command(slash_command, ephemeral = true, guild_only)]
pub async fn list_classes(ctx: PoiseContext<'_>) -> Result<()> {
    let guild_id = ctx.guild().unwrap().id;
    let roles = guild_id.roles(ctx).await?;

    let user_roles = guild_id
        .member(ctx, ctx.author().id)
        .await?
        .roles(ctx)
        .unwrap_or_default();

    let mut message_text = String::from(
        "### Classes:\nJoin any of them with `/join_class <role name>` (may need college prefix)\n",
    );

    let mut last_prefix = None;

    for role in get_class_roles(roles) {
        let (prefix, number) = role.name.split_once(" ").unwrap();
        if last_prefix.as_ref().map(|p| p != prefix).unwrap_or(true) {
            last_prefix = Some(prefix.to_string());
            message_text.push_str(&format!("\n**{prefix}**: "));
        }

        if user_roles.contains(&role) {
            message_text.push_str(&format!("*`{}`* ", number));
        } else {
            message_text.push_str(&format!("`{}` ", number));
        }
    }

    message_text.push_str("\n-# (italicized means you're in it)");

    if message_text.len() > 1024 {
        message_text.truncate(1021);
        message_text.push_str("...");
    }

    ctx.say(message_text).await?;

    Ok(())
}

#[poise::command(prefix_command, guild_only, check = is_stefan)]
pub async fn healthcheck_classes(ctx: PoiseContext<'_>) -> Result<()> {
    let guild_id = ctx.guild().unwrap().id;

    let roles: FxHashSet<_> = get_class_roles(guild_id.roles(ctx).await?)
        .map(|role| role.name)
        .collect();

    let channels = guild_id.channels(ctx).await?;
    let mut roles_from_channels = FxHashSet::default();

    for channel in channels.values() {
        if channel.kind != ChannelType::Category {
            continue;
        }

        let Some(role_from_channel_name) = channel.name.split_once(" - ").map(|(r, _)| r) else {
            continue;
        };

        roles_from_channels.insert(role_from_channel_name.to_string());
    }

    let missing_from_roles: FxHashSet<_> = roles.difference(&roles_from_channels).collect();
    let missing_from_classes: FxHashSet<_> = roles_from_channels.difference(&roles).collect();

    let mut output = match (missing_from_roles, missing_from_classes) {
        (a, b) if a.is_empty() && b.is_empty() => "All good! No discrepancies found.".to_string(),
        (a, b) => {
            let mut output = String::new();
            if !a.is_empty() {
                writeln!(&mut output, "Roles missing categories:").unwrap();
                for role in a {
                    writeln!(&mut output, "- {role}").unwrap();
                }
            }

            if !b.is_empty() {
                writeln!(&mut output, "Categories missing roles:").unwrap();
                for role in b {
                    writeln!(&mut output, "- {role}").unwrap();
                }
            }

            output
        }
    };

    tracing::info!("Healthcheck classes output:\n{}", output);

    output.truncate(2000);

    ctx.say(output).await?;

    Ok(())
}

/// List the classes you're in via roles
#[poise::command(slash_command, ephemeral = true, guild_only)]
pub async fn my_classes(ctx: PoiseContext<'_>) -> Result<()> {
    let user = ctx.author();
    let guild_id = ctx.guild().unwrap().id;
    let roles = guild_id.roles(ctx).await?;

    let Some(user_roles) = guild_id.member(ctx, user.id).await?.roles(ctx) else {
        ctx.say("You don't have any roles?").await?;

        return Ok(());
    };

    let user_roles_formatted = get_class_roles(roles)
        .filter(|role| user_roles.contains(role))
        .map(|role| role.name)
        .collect_vec();

    let message_text = if user_roles_formatted.is_empty() {
        String::from("You don't have any class roles.")
    } else {
        let mut text = String::from("Your classes:\n");
        for role_str in user_roles_formatted {
            writeln!(&mut text, "- `{role_str}`")?;
        }

        text
    };

    ctx.say(message_text).await?;

    Ok(())
}

enum GetRoleResult {
    Found(RoleId),
    MultipleFound(Vec<Role>),
    NotFound,
}

async fn get_role(ctx: PoiseContext<'_>, course: &CourseIdent) -> Result<GetRoleResult> {
    let guild_id = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    let roles = guild_id.roles(ctx).await?;

    let joinable_roles = get_class_roles(roles)
        .filter(|Role { name, .. }| course.spaced_string_starts_with(name.as_str()))
        .collect_vec();

    if joinable_roles.is_empty() {
        return Ok(GetRoleResult::NotFound);
    }

    if joinable_roles.len() > 1 {
        return Ok(GetRoleResult::MultipleFound(joinable_roles));
    }

    Ok(GetRoleResult::Found(joinable_roles.first().unwrap().id))
}

const MOD_ROLE_ID: RoleId = RoleId::new(1192863993883279532);

#[poise::command(
    slash_command,
    required_permissions = "MANAGE_CHANNELS",
    guild_only,
    description_localized("en-US", "Creates a class category")
)]
pub async fn create_class_category(
    ctx: PoiseContext<'_>,
    #[description = "The course identifier (auto adds \"CS\" if unspecified)"] course_id: String,
    #[description = "Whether to skip creating the role, checking it exists instead"]
    skip_creating_role: Option<bool>,
) -> Result<()> {
    let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;

    let Ok(course) = CourseIdent::try_from(course_id.as_str()) else {
        ctx.reply_ephemeral(format!("Please provide a valid course, got `{course_id}`"))
            .await?;
        return Ok(());
    };

    let channels = guild.channels(ctx).await?;

    for channel in channels.values() {
        if channel.kind != ChannelType::Category {
            continue;
        }

        if course.spaced_string_starts_with(&channel.name) {
            ctx.say(format!(
                "Category/channels for {} already seem to exist! See <#{}>",
                course_id, channel.id,
            ))
            .await?;
            return Ok(());
        }
    }

    let role_name = course.get_spaced();

    let (category_name, channel_description) = get_course(&course)
        .map(|course| {
            let mut category_name = format!("{role_name} - {}", course.long_name);
            category_name.truncate(100);
            let mut channel_description = course.description;
            channel_description.truncate(1024);

            (Some(category_name), Some(channel_description))
        })
        .unwrap_or((None, None));

    let role = if skip_creating_role.unwrap_or(false) {
        let roles = guild.roles(ctx).await?;
        let existing_role = roles.values().find(|r| r.name == role_name);
        if let Some(existing_role) = existing_role {
            existing_role.clone()
        } else {
            ctx.say(format!(
                "Role `{}` does not exist, but you said don't create a role. Exiting.",
                role_name
            ))
            .await?;
            return Ok(());
        }
    } else {
        guild
            .create_role(ctx, EditRole::new().name(&role_name))
            .await
            .wrap_err("Couldn't create role")?
    };

    let category = guild
        .create_channel(
            ctx,
            CreateChannel::new(category_name.unwrap_or(role_name))
                .kind(ChannelType::Category)
                .permissions(vec![
                    PermissionOverwrite {
                        allow: Permissions::VIEW_CHANNEL,
                        deny: Permissions::empty(),
                        kind: PermissionOverwriteType::Role(role.id),
                    },
                    PermissionOverwrite {
                        allow: Permissions::VIEW_CHANNEL,
                        deny: Permissions::empty(),
                        kind: PermissionOverwriteType::Role(MOD_ROLE_ID),
                    },
                    PermissionOverwrite {
                        allow: Permissions::empty(),
                        deny: Permissions::VIEW_CHANNEL,
                        kind: PermissionOverwriteType::Role(guild.everyone_role()),
                    },
                ]),
        )
        .await
        .wrap_err("Couldn't create category")?;

    let number = course.number();

    guild
        .create_channel(
            ctx,
            CreateChannel::new(format!("{number}-resources"))
                .kind(ChannelType::Text)
                .category(category.id),
        )
        .await
        .wrap_err("Couldn't create resources channel")?;

    guild
        .create_channel(
            ctx,
            CreateChannel::new(format!("{number}-general"))
                .kind(ChannelType::Text)
                .category(category.id)
                .topic(channel_description.unwrap_or_default()),
        )
        .await
        .wrap_err("Couldn't create general channel")?;

    tracing::info!("Created class category {}", course.as_str());

    ctx.reply_ephemeral("Success!").await?;
    Ok(())
}

#[poise::command(
    slash_command,
    required_permissions = "MANAGE_CHANNELS",
    guild_only,
    description_localized("en-US", "Deletes a class category")
)]
pub async fn delete_class_category(
    _ctx: PoiseContext<'_>,
    #[description = "The class identifier, eg. for CS2420 put in \"CS2420\" or \"2420\""]
    _identifier: String,
) -> Result<()> {
    // let guild = ctx.guild().ok_or_eyre("Couldn't get guild")?.id;
    // let channels = guild.channels(ctx).await?;

    // let category_regex = format!("^CS {}", number);
    // let pattern = Regex::new(&category_regex)?;

    // let gotten_channels = channels
    //     .values()
    //     .find(|channel| pattern.is_match(&channel.name));

    // let Some(category_channel) = gotten_channels else {
    //     ctx.say("Could not find category channel!").await?;
    //     return Ok(());
    // };

    // let children_guild_channels = channels
    //     .values()
    //     .filter(|guild_channel| matches!(guild_channel.parent_id, Some(parent) if parent.eq(&category_channel.id)));

    // let role_id = get_role(ctx, &number).await?;

    // category_channel.delete(ctx).await?;
    // for guild_channel in children_guild_channels {
    //     guild_channel.delete(ctx).await?;
    // }
    // guild.delete_role(ctx, role_id).await?;

    // ctx.say("Success!").await?;
    Ok(())
}

#[poise::command(
    prefix_command,
    ephemeral = true,
    guild_only,
    check = is_stefan
)]
pub async fn debug_print_channel_names(ctx: PoiseContext<'_>) -> Result<()> {
    for channel in get_all_class_general_channels(&ctx).unwrap_or_default() {
        tracing::info!("Found channel {}", channel.name);
    }

    ctx.reply("Process started!").await?;

    Ok(())
}

#[poise::command(
    slash_command,
    required_permissions = "MANAGE_CHANNELS",
    ephemeral = true,
    guild_only,
    check = is_stefan
)]
pub async fn reset_all_class_categories(ctx: PoiseContext<'_>) -> Result<()> {
    ctx.defer_ephemeral().await?;

    for channel in get_all_class_general_channels(&ctx).unwrap_or_default() {
        let ctx = ctx.serenity_context().clone();

        tokio::spawn(async move {
            if let Err(e) = delete_and_replace_channel(ctx, channel).await {
                tracing::warn!("Failed to delete and replace channel: {:?}", e);
            }
        });
    }

    ctx.reply("Process started!").await?;

    Ok(())
}

pub(crate) fn get_all_class_general_channels(ctx: &PoiseContext<'_>) -> Option<Vec<GuildChannel>> {
    ctx.guild().map(|guild| {
        guild
            .channels
            .values()
            .filter(|c| c.parent_id.is_some_and(is_parent_category_ok_to_automate))
            .filter(|c| !c.name.contains("-resources"))
            .cloned()
            .collect::<Vec<_>>()
    })
}

pub(crate) fn is_parent_category_ok_to_automate(parent_id: impl Into<u64>) -> bool {
    !matches!(
        parent_id.into(),
        1105656100856025138
            | 1105654574175502376
            | 1105657058424016926
            | 1281025666694910086
            | 1200645054436491356
            | 1105659164119801907
    )
}

async fn delete_and_replace_channel(ctx: Context, channel: GuildChannel) -> Result<()> {
    let Some(parent_id) = channel.parent_id else {
        return Ok(());
    };

    let Some(topic) = channel.topic else {
        return Ok(());
    };

    channel
        .guild_id
        .create_channel(
            &ctx,
            CreateChannel::new(channel.name)
                .category(parent_id)
                .kind(ChannelType::Text)
                .permissions(channel.permission_overwrites)
                .position(channel.position)
                .topic(topic),
        )
        .await?;

    channel.id.delete(&ctx).await?;

    Ok(())
}

/// Reset the current channel you're in - aka - delete all the messages
#[poise::command(
    slash_command,
    required_permissions = "MANAGE_CHANNELS",
    ephemeral = true,
    guild_only,
    check = is_stefan
)]
pub async fn reset_class_category(ctx: PoiseContext<'_>) -> Result<()> {
    let channel_id = ctx.channel_id();

    let Some(channel) = channel_id.to_channel(&ctx).await?.guild() else {
        ctx.say("This channel is not in a guild").await?;
        return Ok(());
    };

    let Some(parent_id) = channel.parent_id else {
        ctx.say("This channel is not in a class category").await?;

        return Ok(());
    };

    if !is_parent_category_ok_to_automate(parent_id) {
        ctx.say("Channels in this category are not resettable")
            .await?;

        return Ok(());
    }

    let Some(topic) = channel.topic else {
        ctx.say("This channel has no topic, which is weird, so, cancelling for safety.")
            .await?;
        return Ok(());
    };

    channel
        .guild_id
        .create_channel(
            &ctx,
            CreateChannel::new(channel.name)
                .category(parent_id)
                .kind(ChannelType::Text)
                .permissions(channel.permission_overwrites)
                .position(channel.position)
                .topic(topic),
        )
        .await?;

    ctx.say("Success! Deleting current channel in 5 seconds!")
        .await?;

    tokio::time::sleep(Duration::from_secs(5)).await;

    channel_id.delete(&ctx).await?;

    Ok(())
}

/// Join a class. Enter the CS identifier, eg. for CS2420 put in "CS2420" or "2420"
#[poise::command(slash_command, rename = "join_class", ephemeral = true, guild_only)]
pub async fn add_class_role(
    ctx: PoiseContext<'_>,
    #[description = "The course identifier (auto adds \"CS\" if unspecified)"] course_id: String,
) -> Result<()> {
    let user = ctx.author();
    let guild_id = ctx.guild_id().unwrap();
    let author = get_member(ctx).await?;

    let Ok(course) = CourseIdent::try_from(course_id.as_str()) else {
        ctx.reply_ephemeral(format!("Please provide a valid course, got `{course_id}`"))
            .await?;
        return Ok(());
    };

    match get_role(ctx, &course).await? {
        GetRoleResult::Found(role_id) => {
            if user.has_role(ctx, guild_id, &role_id).await? {
                ctx.say("You already have that class role! If the class channels are missing, let the mods know.").await?;
                return Ok(());
            }

            author
                .add_role(ctx, role_id)
                .await
                .wrap_err("Couldn't add role")?;

            ctx.say("Joined class!").await?;
        }
        GetRoleResult::MultipleFound(roles) => {
            let mut message_text = format!("Multiple classes found with search `{course_id}`\n");
            for role in roles {
                if user.has_role(ctx, guild_id, &role).await? {
                    message_text.push_str(&format!("*`{}`* ", role.name));
                } else {
                    message_text.push_str(&format!("`{}` ", role.name));
                }
            }
            ctx.say(message_text).await?;
        }
        GetRoleResult::NotFound => {
            ctx.say(format!(
                "That class does not exist, maybe try <#1144767401481736374>?`{course_id}`"
            ))
            .await?;
        }
    }

    Ok(())
}

/// Leave a class. Enter the CS identifier, eg. for CS2420 put in "CS2420" or "2420"
#[poise::command(slash_command, rename = "leave_class", ephemeral = true, guild_only)]
pub async fn remove_class_role(
    ctx: PoiseContext<'_>,
    #[description = "The course identifier (auto adds \"CS\" if unspecified)"] course_id: String,
) -> Result<()> {
    let author = get_member(ctx).await?;

    let Ok(course) = CourseIdent::try_from(course_id.as_str()) else {
        ctx.reply_ephemeral(format!("Please provide a valid course, got `{course_id}`"))
            .await?;
        return Ok(());
    };

    match get_role(ctx, &course).await? {
        GetRoleResult::Found(role_id) => {
            author
                .remove_role(ctx, role_id)
                .await
                .wrap_err("Couldn't remove role")?;

            ctx.say("Left class!").await?;
        }
        GetRoleResult::MultipleFound(roles) => {
            let mut message_text = format!("Multiple classes found with search `{course_id}`\n");
            for role in roles {
                message_text.push_str(&format!("`{}` ", role.name));
            }
            ctx.say(message_text).await?;
        }
        GetRoleResult::NotFound => {
            ctx.say(format!(
                "Could not find class with identifier `{course_id}`"
            ))
            .await?;
        }
    }

    Ok(())
}

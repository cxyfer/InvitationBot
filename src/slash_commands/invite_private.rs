use crate::utils::config::InviteLimit; // Removed unused AllowedRole import
use crate::{t, Context, Error};
use chrono::{Duration, Utc};
use poise::serenity_prelude::{CreateEmbed, CreateEmbedFooter};
use poise::CreateReply;
use std::collections::HashMap;
use uuid::Uuid;

/// Create a private invite link (anonymous inviter)
#[poise::command(slash_command, guild_only)]
pub async fn invite_private(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(id) => id,
        None => {
            eprintln!("Guild ID not found.");
            return Ok(()); // Exit if guild_id is not found
        }
    };

    let guild = match ctx.guild() {
        Some(g) => g.clone(),
        None => {
            eprintln!("Guild not found.");
            return Ok(()); // Handle the error appropriately
        }
    };

    let member = ctx.author_member().await.unwrap_or_default();
    let locale = ctx.data().config.get_guild_locale(&guild_id.to_string());

    // Validate guild configuration
    let guild_config = match ctx
        .data()
        .config
        .guilds
        .allowed
        .iter()
        .find(|g| g.id == guild_id.to_string())
    {
        Some(config) => config,
        None => {
            // Use a generic error key for now, will refine with specific i18n keys later
            send_error_embed(ctx, locale, "commands.invite_private.errors.server_not_allowed")
                .await?;
            return Ok(());
        }
    };

    // Validate member join date
    let min_stay_duration = Duration::seconds(
        guild_config
            .min_member_age
            .unwrap_or(ctx.data().config.bot.default_min_member_age) as i64,
    );

    if let Some(join_date) = member.joined_at {
        let join_date_utc = join_date.naive_utc().and_utc();
        let joined_time = Utc::now() - join_date_utc;
        if joined_time < min_stay_duration {
            let params =
                create_not_long_enough_params(min_stay_duration.num_days(), joined_time.num_days());
            // Use a generic error key for now
            send_not_long_enough_embed(
                ctx,
                locale,
                "commands.invite_private.errors.not_long_enough",
                params,
            )
            .await?;
            return Ok(());
        }
    } else {
        send_error_embed(ctx, locale, "commands.invite_private.errors.join_date_not_found")
            .await?;
        return Ok(());
    }

    // Validate member roles for private invite permissions
    let (_role_with_limit, private_limit) = match member.roles.iter().find_map(|role_id| { // Prefixed unused variable with _
        guild_config.allowed_roles.iter().find_map(|allowed_role| {
            if allowed_role.id == role_id.to_string() {
                allowed_role.private_invite_limit.as_ref().map(|limit| (allowed_role, limit))
            } else {
                None
            }
        })
    }) {
        Some((role, limit)) => (role, limit), // Found role with private limit configured
        None => {
            send_error_embed(ctx, locale, "commands.invite_private.errors.missing_permissions")
                .await?;
            return Ok(());
        }
    };

    // Check private invite usage limit (using placeholder for new db function)
    let used_private_invites = crate::utils::db::count_used_private_invites(
        &ctx.data().db,
        &ctx.author().id.to_string(),
        &guild_id.to_string(),
        private_limit.days,
    )
    .await?;

    if used_private_invites >= private_limit.count as i64 {
        let params = create_limit_params(private_limit, used_private_invites);
        // Use a generic error key for now
        send_limit_reached_embed(
            ctx,
            locale,
            "commands.invite_private.errors.limit_reached",
            params,
        )
        .await?;
        return Ok(());
    }

    // Create invite link (without recording inviter in the main invites table)
    let invite_id = Uuid::new_v4().to_string();

    // 創建私人邀請
    crate::utils::db::create_invite(
        &ctx.data().db,
        &invite_id,
        &guild_id.to_string(),
        &ctx.author().id.to_string(),
        true, // 設為私人邀請
    )
    .await?;

    // Record private invite usage (using placeholder for new db function)
    crate::utils::db::record_private_invite_usage(
        &ctx.data().db,
        &guild_id.to_string(),
        &ctx.author().id.to_string(),
    )
    .await?;

    let bot_invite_url = format!(
        "{}/invite/{}",
        ctx.data().config.server.external_url,
        invite_id
    );

    let guild_name = guild.name.clone();
    send_success_embed(
        ctx, // Use a generic success key for now
        locale,
        "commands.invite_private.success",
        guild_name,
        private_limit,
        used_private_invites + 1,
        bot_invite_url,
        guild.icon_url(),
    )
    .await?;

    Ok(())
}

async fn send_error_embed(ctx: Context<'_>, locale: &str, error_key: &str) -> Result<(), Error> {
    let embed = CreateEmbed::default()
        .title(t!(locale, format!("{}.title", error_key).as_str()))
        .description(t!(locale, format!("{}.description", error_key).as_str()))
        .color(0xFF3333)
        .footer(CreateEmbedFooter::new(t!(
            locale,
            format!("{}.footer", error_key).as_str()
        )));

    ctx.send(CreateReply::default().embed(embed).ephemeral(true))
        .await?;
    Ok(())
}

async fn send_limit_reached_embed(
    ctx: Context<'_>,
    locale: &str,
    error_key_base: &str, // e.g., "commands.invite_private.errors.limit_reached"
    params: HashMap<&str, String>,
) -> Result<(), Error> {
    let embed = CreateEmbed::default()
        .title(t!(locale, format!("{}.title", error_key_base).as_str()))
        .description(format!(
            "{}\n\n**{}**:\n• {}\n• {}",
            t!(
                locale,
                format!("{}.description", error_key_base).as_str(),
                params.clone()
            ),
            t!(locale, format!("{}.status", error_key_base).as_str()), // Assuming similar structure
            t!(
                locale,
                format!("{}.used", error_key_base).as_str(),
                params.clone()
            ),
            t!(
                locale,
                format!("{}.remaining", error_key_base).as_str(),
                params
            ),
        ))
        .color(0xFF3333)
        .footer(CreateEmbedFooter::new(t!(
            locale,
            format!("{}.footer", error_key_base).as_str()
        )));

    ctx.send(CreateReply::default().embed(embed).ephemeral(true))
        .await?;
    Ok(())
}

async fn send_not_long_enough_embed(
    ctx: Context<'_>,
    locale: &str,
    error_key_base: &str, // e.g., "commands.invite_private.errors.not_long_enough"
    params: HashMap<&str, String>,
) -> Result<(), Error> {
    let embed = CreateEmbed::default()
        .title(t!(locale, format!("{}.title", error_key_base).as_str()))
        .description(t!(
            locale,
            format!("{}.description", error_key_base).as_str(),
            params.clone()
        ))
        .color(0xFF3333)
        .footer(CreateEmbedFooter::new(t!(
            locale,
            format!("{}.footer", error_key_base).as_str(),
            params.clone()
        )));

    ctx.send(CreateReply::default().embed(embed).ephemeral(true))
        .await?;
    Ok(())
}

async fn send_success_embed(
    ctx: Context<'_>,
    locale: &str,
    success_key_base: &str, // e.g., "commands.invite_private.success"
    guild_name: String,
    private_limit: &InviteLimit,
    used_private_invites: i64,
    bot_invite_url: String,
    guild_icon_url: Option<String>,
) -> Result<(), Error> {
    let params = create_success_params(private_limit, used_private_invites, &guild_name);
    let embed = CreateEmbed::default()
        .title(t!(locale, format!("{}.title", success_key_base).as_str()))
        .description(format!(
            "{}\n\n{}\n\n**{}**:\n• {}\n• {}",
            t!(
                locale,
                format!("{}.description", success_key_base).as_str(),
                params.clone()
            ),
            bot_invite_url,
            t!(locale, format!("{}.limits", success_key_base).as_str()), // Assuming similar structure
            t!(
                locale,
                format!("{}.invites_per_days", success_key_base).as_str(),
                params.clone()
            ),
            t!(locale, format!("{}.used_remaining", success_key_base).as_str(), params),
        ))
        .color(0x4CACEE)
        .thumbnail(guild_icon_url.unwrap_or_default())
        .footer(CreateEmbedFooter::new(t!(
            locale,
            format!("{}.footer", success_key_base).as_str()
        )));

    ctx.send(CreateReply::default().embed(embed).ephemeral(true))
        .await?;
    Ok(())
}

fn create_limit_params(limit: &InviteLimit, used_invites: i64) -> HashMap<&str, String> {
    let mut params = HashMap::new();
    params.insert("count", limit.count.to_string());
    params.insert("days", limit.days.to_string());
    params.insert("used", used_invites.to_string());
    params.insert(
        "remaining",
        (limit.count as i64 - used_invites).to_string(),
    );
    params
}

fn create_not_long_enough_params<'a>(days: i64, remaining: i64) -> HashMap<&'a str, String> {
    let mut params = HashMap::new();
    params.insert("days", days.to_string());
    params.insert("remaining", remaining.to_string());
    params
}

fn create_success_params<'a>(
    limit: &'a InviteLimit,
    used_invites: i64,
    guild_name: &'a str,
) -> HashMap<&'a str, String> {
    let mut params = HashMap::new();
    params.insert("guild", guild_name.to_string());
    params.insert("count", limit.count.to_string());
    params.insert("days", limit.days.to_string());
    params.insert("used", used_invites.to_string());
    params.insert(
        "remaining",
        (limit.count as i64 - used_invites).to_string(),
    );
    params
}
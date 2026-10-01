//! Liveness reporting for the container healthcheck (`bot --healthcheck`) and
//! the `/status` command.
//!
//! While every shard is connected to the gateway and the database answers, the
//! bot writes the current time to a heartbeat file every [`CHECK_INTERVAL`].
//! `bot --healthcheck` runs as a separate process and only checks that file's age.

use crate::{
    commands::is_stefan,
    data::{PoiseContext, with_db},
    utils::SendReplyEphemeral,
};
use color_eyre::eyre::{Context, Result, bail, eyre};
use poise::serenity_prelude::{ConnectionStage, ShardManager};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CHECK_INTERVAL: Duration = Duration::from_secs(30);
/// Three missed checks in a row.
const MAX_HEARTBEAT_AGE: Duration = Duration::from_secs(90);
const DB_TIMEOUT: Duration = Duration::from_secs(5);

static STARTED: LazyLock<Instant> = LazyLock::new(Instant::now);

fn heartbeat_path() -> PathBuf {
    std::env::var_os("KINGFISHER_HEARTBEAT_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("kingfisher-heartbeat"))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Marks startup and removes any heartbeat left by a previous run, so a
/// restarted container isn't reported healthy before it has connected.
pub fn reset() {
    LazyLock::force(&STARTED);
    let _ = std::fs::remove_file(heartbeat_path());
}

/// For `bot --healthcheck`: returns the heartbeat's age, or an error if it is
/// missing or older than [`MAX_HEARTBEAT_AGE`].
pub fn check_heartbeat() -> Result<Duration> {
    check_heartbeat_at(&heartbeat_path())
}

fn check_heartbeat_at(path: &Path) -> Result<Duration> {
    let written: u64 = std::fs::read_to_string(path)
        .wrap_err_with(|| format!("no heartbeat at {}", path.display()))?
        .trim()
        .parse()
        .wrap_err("malformed heartbeat")?;

    let age = Duration::from_secs(unix_now().saturating_sub(written));
    if age > MAX_HEARTBEAT_AGE {
        bail!("last heartbeat was {}s ago", age.as_secs());
    }

    Ok(age)
}

struct Report {
    shards: Vec<(u32, ConnectionStage, Option<Duration>)>,
    /// Schema version and file size in bytes.
    db: Result<(u32, u64)>,
}

impl Report {
    fn is_healthy(&self) -> bool {
        !self.shards.is_empty()
            && self
                .shards
                .iter()
                .all(|(_, stage, _)| *stage == ConnectionStage::Connected)
            && self.db.is_ok()
    }
}

async fn check(shard_manager: &ShardManager) -> Report {
    let mut shards: Vec<_> = shard_manager
        .runners
        .lock()
        .await
        .iter()
        .map(|(id, runner)| (id.0, runner.stage, runner.latency))
        .collect();
    shards.sort_by_key(|(id, ..)| *id);

    let query = with_db(|conn| {
        Ok(conn.query_row(
            "SELECT user_version, page_count * page_size \
             FROM pragma_user_version, pragma_page_count, pragma_page_size",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    });
    let db = tokio::time::timeout(DB_TIMEOUT, query)
        .await
        .unwrap_or_else(|_| Err(eyre!("no response within {DB_TIMEOUT:?}")));

    Report { shards, db }
}

/// Checks health every [`CHECK_INTERVAL`] and writes the heartbeat while healthy.
pub async fn run_heartbeat(shard_manager: Arc<ShardManager>) {
    let path = heartbeat_path();
    let mut was_healthy = true;
    let mut interval = tokio::time::interval(CHECK_INTERVAL);

    loop {
        interval.tick().await;
        let report = check(&shard_manager).await;

        if report.is_healthy() {
            if let Err(error) = std::fs::write(&path, unix_now().to_string()) {
                tracing::warn!(?error, path = %path.display(), "failed to write heartbeat");
            }
            if !was_healthy {
                tracing::info!("health check recovered");
            }
        } else if was_healthy {
            tracing::error!(shards = ?report.shards, db = ?report.db, "health check failing");
        }

        was_healthy = report.is_healthy();
    }
}

fn format_duration(duration: Duration) -> String {
    humantime::format_duration(Duration::from_secs(duration.as_secs())).to_string()
}

/// Show the bot's uptime, gateway connection and database status
#[poise::command(slash_command, ephemeral = true, check = is_stefan)]
pub async fn status(ctx: PoiseContext<'_>) -> Result<()> {
    let report = check(&ctx.framework().shard_manager()).await;

    let mut content = format!(
        "**Kingfisher {}**: {}\nUptime: {}\n",
        env!("CARGO_PKG_VERSION"),
        if report.is_healthy() {
            "healthy"
        } else {
            "unhealthy"
        },
        format_duration(STARTED.elapsed()),
    );

    for (id, stage, latency) in &report.shards {
        let latency = latency.map_or("unknown".to_owned(), |latency| {
            format!("{} ms", latency.as_millis())
        });
        content.push_str(&format!("Shard {id}: {stage}, latency {latency}\n"));
    }

    match &report.db {
        Ok((schema_version, bytes)) => content.push_str(&format!(
            "Database: ok, schema v{schema_version}, {} KiB\n",
            bytes / 1024
        )),
        Err(error) => content.push_str(&format!("Database: {error}\n")),
    }

    content.push_str(&match check_heartbeat() {
        Ok(age) => format!("Heartbeat: {} ago", format_duration(age)),
        Err(error) => format!("Heartbeat: {error}"),
    });

    ctx.reply_ephemeral(content).await?;

    Ok(())
}

#[cfg(test)]
#[test]
fn heartbeat_age_is_checked() {
    let path =
        std::env::temp_dir().join(format!("kingfisher-heartbeat-test-{}", std::process::id()));

    let _ = std::fs::remove_file(&path);
    assert!(
        check_heartbeat_at(&path).is_err(),
        "a missing heartbeat is unhealthy"
    );

    std::fs::write(&path, (unix_now() - 5).to_string()).unwrap();
    assert_eq!(check_heartbeat_at(&path).unwrap().as_secs(), 5);

    std::fs::write(&path, (unix_now() - 91).to_string()).unwrap();
    assert!(
        check_heartbeat_at(&path).is_err(),
        "a stale heartbeat is unhealthy"
    );

    std::fs::write(&path, "garbage").unwrap();
    assert!(check_heartbeat_at(&path).is_err());

    std::fs::remove_file(&path).unwrap();
}

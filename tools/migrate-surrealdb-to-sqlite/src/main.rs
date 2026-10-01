//! One-off migration from the embedded SurrealDB 3 (RocksDB) database to SQLite.
//!
//! Usage: migrate-surrealdb-to-sqlite <surrealdb-rocksdb-dir> <sqlite-file>
//!
//! The source directory is only read. The SQLite file is written to a temporary
//! path and renamed into place once every row has been copied and verified, so
//! a failed run never leaves a half-migrated database behind.

use rusqlite::{Connection, params};
use std::{
    collections::BTreeMap,
    error::Error,
    path::{Path, PathBuf},
};
use surrealdb::{
    Surreal,
    engine::local::{Db, RocksDb},
    types::{SurrealValue, Value},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Must match `MIGRATIONS` in bot-lib/src/data.rs.
const MIGRATIONS: &[&str] = &[include_str!("../../../bot-lib/migrations/0001_initial.sql")];

/// Tables whose rows are copied into SQLite.
const MIGRATED_TABLES: &[&str] = &[
    "starboard_recent_message",
    "bank_account",
    "yeet_score",
    "message_limit",
    "message_count",
];

/// Singleton rows seeded by the old schema.surrealql. Nothing reads them (the
/// live values come from config.toml), so they are reported but not copied.
const SEED_TABLES: &[&str] = &["bot_settings", "react_settings", "yeet_settings"];

#[derive(SurrealValue)]
struct Change {
    amount: i64,
    reason: String,
}

#[derive(SurrealValue)]
struct BankAccount {
    user_id: i64,
    balance: i64,
    changes: Vec<Change>,
}

#[derive(SurrealValue)]
struct YeetScore {
    user_id: i64,
    count: i64,
}

#[derive(SurrealValue)]
struct MessageLimit {
    user_id: i64,
    guild_id: i64,
    daily_limit: i64,
    imposed_by: Option<i64>,
    created_at: String,
}

#[derive(SurrealValue)]
struct MessageCount {
    user_id: i64,
    guild_id: i64,
    count: i64,
    reset_date: String,
}

struct Data {
    starboard_recent_messages: Vec<i64>,
    bank_accounts: Vec<BankAccount>,
    yeet_scores: Vec<YeetScore>,
    message_limits: Vec<MessageLimit>,
    message_counts: Vec<MessageCount>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let (Some(source), Some(target), None) = (args.next(), args.next(), args.next()) else {
        return Err(
            "usage: migrate-surrealdb-to-sqlite <surrealdb-rocksdb-dir> <sqlite-file>".into(),
        );
    };
    let (source, target) = (PathBuf::from(source), PathBuf::from(target));

    // Connecting to a missing directory would silently create an empty database.
    if !source.join("CURRENT").is_file() {
        return Err(format!("{} is not a RocksDB directory", source.display()).into());
    }
    if target.exists() {
        return Err(format!(
            "{} already exists; refusing to overwrite it",
            target.display()
        )
        .into());
    }

    eprintln!("Reading {}", source.display());
    let db: Surreal<Db> = Surreal::init();
    db.connect::<RocksDb>(source.as_path())
        .await
        .map_err(|error| {
            format!(
                "failed to open {}: {error}; stop the bot first",
                source.display()
            )
        })?;
    db.use_ns("main").use_db("main").await?;

    check_unmigrated_tables(&db).await?;
    let data = read(&db).await?;
    drop(db);

    write(&target, data)?;
    eprintln!("Wrote {}", target.display());
    Ok(())
}

async fn check_unmigrated_tables(db: &Surreal<Db>) -> Result<()> {
    let tables: Vec<String> = db
        .query("RETURN object::keys((INFO FOR DB).tables)")
        .await?
        .check()?
        .take(0)?;

    for table in tables {
        let count: Option<i64> = db
            .query("RETURN count(SELECT VALUE id FROM type::table($table))")
            .bind(("table", table.clone()))
            .await?
            .check()?
            .take(0)?;
        let count = count.unwrap_or(0);
        eprintln!("  {table}: {count} rows");

        if count == 0 || MIGRATED_TABLES.contains(&table.as_str()) {
            continue;
        }

        if SEED_TABLES.contains(&table.as_str()) && count == 1 {
            let row: Option<Value> = db
                .query("SELECT * FROM ONLY type::table($table) LIMIT 1")
                .bind(("table", table.clone()))
                .await?
                .check()?
                .take(0)?;
            eprintln!("    not migrated (unused seed defaults): {row:?}");
            continue;
        }

        return Err(format!(
            "table {table} has {count} rows but is not migrated; refusing to drop data"
        )
        .into());
    }

    Ok(())
}

async fn read(db: &Surreal<Db>) -> Result<Data> {
    let mut response = db
        .query("SELECT VALUE record::id(id) FROM starboard_recent_message")
        .query("SELECT record::id(id) AS user_id, balance, changes FROM bank_account")
        .query("SELECT record::id(id) AS user_id, count FROM yeet_score")
        .query(
            "SELECT user_id, guild_id, daily_limit, imposed_by, \
             time::format(created_at, '%Y-%m-%dT%H:%M:%S%.3fZ') AS created_at \
             FROM message_limit ORDER BY created_at",
        )
        .query("SELECT user_id, guild_id, count, reset_date FROM message_count")
        .await?
        .check()?;

    Ok(Data {
        starboard_recent_messages: response.take(0)?,
        bank_accounts: response.take(1)?,
        yeet_scores: response.take(2)?,
        message_limits: dedupe(
            "message_limit",
            response.take(3)?,
            |row| (row.user_id, row.guild_id),
            // Rows are ordered by creation, so the latest limit wins.
            |_| (),
        ),
        message_counts: dedupe(
            "message_count",
            response.take(4)?,
            |row| (row.user_id, row.guild_id),
            // Keep today's (or the most recent) count.
            |row| (row.reset_date.clone(), row.count),
        ),
    })
}

/// SurrealDB had no unique index on (user_id, guild_id), so concurrent writes
/// may have left duplicates. Keep the row with the greatest `rank` (the last
/// one on ties) and report the rest.
fn dedupe<T, R: Ord>(
    table: &str,
    rows: Vec<T>,
    key: impl Fn(&T) -> (i64, i64),
    rank: impl Fn(&T) -> R,
) -> Vec<T> {
    let mut kept: BTreeMap<(i64, i64), T> = BTreeMap::new();
    for row in rows {
        let row_key = key(&row);
        match kept.get(&row_key) {
            Some(existing) if rank(existing) > rank(&row) => {
                eprintln!("  {table}: dropping duplicate row for (user, guild) {row_key:?}");
            }
            Some(_) => {
                eprintln!("  {table}: dropping duplicate row for (user, guild) {row_key:?}");
                kept.insert(row_key, row);
            }
            None => {
                kept.insert(row_key, row);
            }
        }
    }
    kept.into_values().collect()
}

fn write(target: &Path, data: Data) -> Result<()> {
    let partial = PathBuf::from(format!("{}.partial", target.display()));
    if partial.exists() {
        std::fs::remove_file(&partial)?;
    }

    let mut conn = Connection::open(&partial)?;
    conn.pragma_update(None, "foreign_keys", true)?;

    let tx = conn.transaction()?;
    for migration in MIGRATIONS {
        tx.execute_batch(migration)?;
    }
    tx.pragma_update(None, "user_version", MIGRATIONS.len())?;

    for message_id in &data.starboard_recent_messages {
        tx.execute(
            "INSERT INTO starboard_recent_message (message_id) VALUES (?1)",
            params![message_id],
        )?;
    }

    let mut change_count = 0;
    for account in &data.bank_accounts {
        tx.execute(
            "INSERT INTO bank_account (user_id, balance) VALUES (?1, ?2)",
            params![account.user_id, account.balance],
        )?;
        for change in &account.changes {
            tx.execute(
                "INSERT INTO bank_change (user_id, amount, reason) VALUES (?1, ?2, ?3)",
                params![account.user_id, change.amount, change.reason],
            )?;
            change_count += 1;
        }
    }

    for score in &data.yeet_scores {
        tx.execute(
            "INSERT INTO yeet_score (user_id, count) VALUES (?1, ?2)",
            params![score.user_id, score.count],
        )?;
    }

    for limit in &data.message_limits {
        tx.execute(
            "INSERT INTO message_limit (user_id, guild_id, daily_limit, imposed_by, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                limit.user_id,
                limit.guild_id,
                limit.daily_limit,
                limit.imposed_by,
                limit.created_at
            ],
        )?;
    }

    for count in &data.message_counts {
        tx.execute(
            "INSERT INTO message_count (user_id, guild_id, count, reset_date) VALUES (?1, ?2, ?3, ?4)",
            params![count.user_id, count.guild_id, count.count, count.reset_date],
        )?;
    }

    tx.commit()?;

    let expected = [
        (
            "starboard_recent_message",
            data.starboard_recent_messages.len(),
        ),
        ("bank_account", data.bank_accounts.len()),
        ("bank_change", change_count),
        ("yeet_score", data.yeet_scores.len()),
        ("message_limit", data.message_limits.len()),
        ("message_count", data.message_counts.len()),
    ];
    eprintln!("Verifying {}", partial.display());
    for (table, expected) in expected {
        let actual: usize =
            conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })?;
        eprintln!("  {table}: {actual} rows");
        if actual != expected {
            return Err(format!("{table} has {actual} rows, expected {expected}").into());
        }
    }

    let expected_total: i64 = data
        .bank_accounts
        .iter()
        .map(|account| account.balance)
        .sum();
    let actual_total: i64 = conn.query_row(
        "SELECT coalesce(sum(balance), 0) FROM bank_account",
        [],
        |row| row.get(0),
    )?;
    if actual_total != expected_total {
        return Err(
            format!("bank balances sum to {actual_total}, expected {expected_total}").into(),
        );
    }

    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(format!("integrity check failed: {integrity}").into());
    }
    let foreign_key_violations: usize =
        conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    if foreign_key_violations != 0 {
        return Err(format!("{foreign_key_violations} foreign key violations").into());
    }

    conn.close().map_err(|(_, error)| error)?;
    std::fs::rename(&partial, target)?;
    Ok(())
}

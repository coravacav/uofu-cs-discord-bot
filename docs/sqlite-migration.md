# SurrealDB to SQLite migration

The bot used an embedded SurrealDB 3 database on RocksDB at `db/kingfisher-v3`.
It now uses SQLite at `db/kingfisher.sqlite` (override with
`KINGFISHER_DB_PATH`). The schema lives in `bot-lib/migrations/`; the bot applies
pending migrations at startup and records the count in `PRAGMA user_version`.

`tools/migrate-surrealdb-to-sqlite` copies the old data. It reads the RocksDB
directory with the same SurrealDB version the bot ran (its `Cargo.lock` is
seeded from the last SurrealDB build) and writes a new SQLite file:

| SurrealDB table            | SQLite table(s)                  |
| -------------------------- | -------------------------------- |
| `starboard_recent_message` | `starboard_recent_message`       |
| `bank_account`             | `bank_account`, `bank_change`    |
| `yeet_score`               | `yeet_score`                     |
| `message_limit`            | `message_limit`                  |
| `message_count`            | `message_count`                  |

The exporter refuses to run if any other table has rows, except the singleton
`bot_settings`, `react_settings` and `yeet_settings` rows that the old schema
seeded and nothing read; it prints those instead. SurrealDB had no unique index
on `(user_id, guild_id)`, so duplicate `message_limit`/`message_count` rows are
collapsed (latest limit, most recent count) and reported. The new file is
verified (row counts, balance total, integrity and foreign-key checks) before it
is renamed into place.

## Cutover

Done on 2026-09-30. The steps were:

1. Build the new image while the old bot keeps running: `just build`.
2. Stop the bot: `docker compose stop bot`.
3. Back up the old database: `cp -a db/kingfisher-v3 backups/kingfisher-v3-before-sqlite-<timestamp>`.
4. Migrate:

   ```sh
   cargo run --release --manifest-path tools/migrate-surrealdb-to-sqlite/Cargo.toml -- \
     db/kingfisher-v3 db/kingfisher.sqlite
   ```

5. Start the new image: `just deploy`, then check `just logs`.

Afterwards the old `db/kingfisher-v3`, `db/kingfisher` (SurrealDB 2) and
`kingfisher.db` (sled) were removed from the checkout; their copies remain in
`backups/` (`kingfisher-v3-before-sqlite-20260930-223507`,
`surreal-v2-pristine-before-v3` and `legacy-sled-before-v3`).

## Rollback

Stop the bot, copy `backups/kingfisher-v3-before-sqlite-20260930-223507` back
to `db/kingfisher-v3`, check out the last SurrealDB commit (`1034eb3`) and
`just deploy`. Data written only to SQLite after the cutover must be reconciled
separately.

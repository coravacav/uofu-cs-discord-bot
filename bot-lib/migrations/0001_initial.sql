-- Initial schema, ported from the SurrealDB tables that held data.
-- Discord snowflakes are stored as INTEGER (i64); they fit until the year 2084.

-- Messages that were already posted to (or permanently excluded from) a starboard.
CREATE TABLE starboard_recent_message (
    message_id INTEGER PRIMARY KEY
) STRICT;

CREATE TABLE bank_account (
    user_id INTEGER PRIMARY KEY,
    balance INTEGER NOT NULL DEFAULT 0
) STRICT;

-- Append-only balance history; `id` preserves insertion order.
CREATE TABLE bank_change (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES bank_account (user_id) ON DELETE CASCADE,
    amount INTEGER NOT NULL,
    reason TEXT NOT NULL
) STRICT;

CREATE INDEX bank_change_user_id ON bank_change (user_id, id);

CREATE TABLE yeet_score (
    user_id INTEGER PRIMARY KEY,
    count INTEGER NOT NULL DEFAULT 0 CHECK (count >= 0)
) STRICT;

-- Message limit configuration per user per guild.
-- `imposed_by` is the moderator's user ID, or NULL for a self-imposed limit.
CREATE TABLE message_limit (
    user_id INTEGER NOT NULL,
    guild_id INTEGER NOT NULL,
    daily_limit INTEGER NOT NULL CHECK (daily_limit > 0),
    imposed_by INTEGER,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (user_id, guild_id)
) STRICT;

-- Daily message counts per user per guild. `reset_date` is a Mountain Time
-- "YYYY-MM-DD" date; a row from an earlier day counts as zero.
CREATE TABLE message_count (
    user_id INTEGER NOT NULL,
    guild_id INTEGER NOT NULL,
    count INTEGER NOT NULL DEFAULT 0 CHECK (count >= 0),
    reset_date TEXT NOT NULL,
    PRIMARY KEY (user_id, guild_id)
) STRICT;

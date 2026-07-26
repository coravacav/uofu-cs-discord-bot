# SurrealDB 2 to 3 migration

The bot's SurrealDB 2 database is stored in `db/kingfisher`. SurrealDB 3 cannot
open that storage format directly. The bot therefore defaults to the separate
`db/kingfisher-v3` directory and refuses to open the legacy path.

Perform this migration with the bot stopped. Keep the original directory until
the new version has been deployed and verified.

## Prerequisites

- SurrealDB 2.6.5 CLI/server, named `surreal-v2` below
- SurrealDB 3.2.3 or newer CLI, named `surreal-v3` below
- Enough free space for a database copy and a SurrealQL export

## Export

1. Stop the bot and confirm no process has `db/kingfisher` open.
2. Create a timestamped backup outside `db/`:

   ```sh
   cp -a db/kingfisher backups/surreal-v2-before-v3
   ```

3. Start SurrealDB 2.6.5 against the backup, not the original directory:

   ```sh
   surreal-v2 start \
     --bind 127.0.0.1:8000 \
     --user root \
     --pass change-this-migration-password \
     rocksdb:backups/surreal-v2-before-v3
   ```

4. In another terminal, use the SurrealDB 3 CLI's compatibility exporter:

   ```sh
   surreal-v3 v2 export \
     --v3 \
     --endpoint http://127.0.0.1:8000 \
     --user root \
     --pass change-this-migration-password \
     --namespace main \
     --database main \
     backups/kingfisher-v3.surql
   ```

5. Stop the temporary SurrealDB 2 server.

## Import

1. Start an empty SurrealDB 3.2.3 server at the new path:

   ```sh
   surreal-v3 start \
     --bind 127.0.0.1:8000 \
     --user root \
     --pass change-this-migration-password \
     rocksdb:db/kingfisher-v3
   ```

2. Import the compatibility export:

   ```sh
   surreal-v3 import \
     --endpoint http://127.0.0.1:8000 \
     --user root \
     --pass change-this-migration-password \
     --namespace main \
     --database main \
     backups/kingfisher-v3.surql
   ```

3. Compare table counts and representative records between the v2 backup and
   the v3 database. At minimum, verify `starboard`, `starboarded`,
   `starboard_recent_message`, `message_limit`, and `message_count`.
4. Stop the temporary SurrealDB 3 server before starting the embedded bot.

## Deploy and rollback

The default v3 path is `db/kingfisher-v3`. It can be changed with
`KINGFISHER_SURREALDB_PATH`, but the bot intentionally rejects
`db/kingfisher`.

After deployment, verify startup schema application, starboard deduplication,
and message-limit reads and writes.

To roll back, stop the v3 bot, deploy the last SurrealDB 2 build, and point it
at the untouched `db/kingfisher` directory. Data written only after the v3
cutover must be reconciled separately before rollback.

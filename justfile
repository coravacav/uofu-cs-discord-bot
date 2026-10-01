# Docker helpers for the Kingfisher bot. All recipes use the compose project
# pinned in docker-compose.yml (name: kingfisher), so this, plain
# `docker compose` and Portainer all see the same stack.

compose := "docker compose -f docker-compose.yml"
service := "bot"
db_file := "db/kingfisher.sqlite"

# List commands
default:
    @just --list

# Build the image (cached cargo deps, only changed crates recompile)
build:
    {{compose}} build

# Start the bot (without rebuilding)
up:
    {{compose}} up -d

# Rebuild and (re)start the bot with the new image
deploy: build
    {{compose}} up -d --force-recreate
    @{{compose}} ps

# Stop and remove the container
down:
    {{compose}} down

# Restart the container (same image)
restart:
    {{compose}} restart {{service}}

# Follow logs (last 200 lines)
logs:
    {{compose}} logs -f --tail=200 {{service}}

# Open a shell in the running container
shell:
    {{compose}} exec {{service}} /bin/bash

# Show container status
status:
    @{{compose}} ps -a
    @docker inspect -f 'restarts={{"{{"}}.RestartCount{{"}}"}} started={{"{{"}}.State.StartedAt{{"}}"}} health={{"{{"}}if .State.Health{{"}}"}}{{"{{"}}.State.Health.Status{{"}}"}}{{"{{"}}else{{"}}"}}none{{"{{"}}end{{"}}"}}' kingfisher-bot 2>/dev/null || true

# Show the healthcheck state and its last results
health:
    @docker inspect -f '{{"{{"}}.State.Health.Status{{"}}"}}{{"{{"}}range .State.Health.Log{{"}}"}}{{"{{"}}println{{"}}"}}{{"{{"}}.End.Format "15:04:05"{{"}}"}} exit={{"{{"}}.ExitCode{{"}}"}} {{"{{"}}.Output{{"}}"}}{{"{{"}}end{{"}}"}}' kingfisher-bot

# Run a query against the live database, or open a sqlite3 shell with no query
sql query="":
    {{compose}} exec {{service}} sqlite3 -box {{db_file}} {{ if query == "" { "" } else { quote(query) } }}

# Check the live database's integrity and foreign keys
db-check:
    {{compose}} exec -T {{service}} sqlite3 {{db_file}} 'PRAGMA integrity_check; PRAGMA foreign_key_check;'

# Snapshot db/kingfisher.sqlite to backups/ (online backup, the bot keeps running)
backup-db:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p backups
    dest="backups/kingfisher-$(date +%Y%m%d-%H%M%S).sqlite"
    if [ -n "$({{compose}} ps -q --status running {{service}})" ]; then
        # Back up from inside the container: SQLite's file locks don't reach across the Docker VM.
        {{compose}} exec -T {{service}} sqlite3 {{db_file}} ".backup '$dest'"
    else
        sqlite3 {{db_file}} ".backup '$dest'"
    fi
    echo "backed up to $dest"

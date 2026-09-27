# Docker helpers for the Kingfisher bot. All recipes use the compose project
# pinned in docker-compose.yml (name: kingfisher), so this, plain
# `docker compose` and Portainer all see the same stack.

compose := "docker compose -f docker-compose.yml"
service := "bot"
db_dir := "db/kingfisher-v3"

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
    @docker inspect -f 'restarts={{"{{"}}.RestartCount{{"}}"}} started={{"{{"}}.State.StartedAt{{"}}"}}' kingfisher-bot 2>/dev/null || true

# Copy db/kingfisher-v3 to backups/ (stops the bot briefly for a consistent copy)
backup-db:
    #!/usr/bin/env bash
    set -u
    mkdir -p backups
    dest="backups/kingfisher-v3-$(date +%Y%m%d-%H%M%S)"
    running=$({{compose}} ps -q --status running {{service}})
    if [ -n "$running" ]; then echo "stopping bot for consistent backup..."; {{compose}} stop {{service}}; fi
    cp -a {{db_dir}} "$dest" && echo "backed up to $dest"
    status=$?
    if [ -n "$running" ]; then {{compose}} start {{service}}; fi
    exit $status

# Docker helpers for the Kingfisher bot. All targets use the compose project
# pinned in docker-compose.yml (name: kingfisher), so this, plain
# `docker compose` and Portainer all see the same stack.

COMPOSE := docker compose -f docker-compose.yml
SERVICE := bot
DB_DIR  := db/kingfisher-v3
STAMP   := $(shell date +%Y%m%d-%H%M%S)

.DEFAULT_GOAL := help
.PHONY: help build up deploy down restart logs shell status backup-db

help: ## List commands
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*## "} {printf "  make %-10s %s\n", $$1, $$2}'

build: ## Build the image (cached cargo deps, only changed crates recompile)
	$(COMPOSE) build

up: ## Start the bot (without rebuilding)
	$(COMPOSE) up -d

deploy: build ## Rebuild and (re)start the bot with the new image
	$(COMPOSE) up -d --force-recreate
	@$(COMPOSE) ps

down: ## Stop and remove the container
	$(COMPOSE) down

restart: ## Restart the container (same image)
	$(COMPOSE) restart $(SERVICE)

logs: ## Follow logs (last 200 lines)
	$(COMPOSE) logs -f --tail=200 $(SERVICE)

shell: ## Open a shell in the running container
	$(COMPOSE) exec $(SERVICE) /bin/bash

status: ## Show container status
	@$(COMPOSE) ps -a
	@docker inspect -f 'restarts={{.RestartCount}} started={{.State.StartedAt}}' kingfisher-bot 2>/dev/null || true

backup-db: ## Copy db/kingfisher-v3 to backups/ (stops the bot briefly for a consistent copy)
	@mkdir -p backups
	@running=$$($(COMPOSE) ps -q --status running $(SERVICE)); \
	if [ -n "$$running" ]; then echo "stopping bot for consistent backup..."; $(COMPOSE) stop $(SERVICE); fi; \
	cp -a $(DB_DIR) backups/kingfisher-v3-$(STAMP) && echo "backed up to backups/kingfisher-v3-$(STAMP)"; \
	status=$$?; \
	if [ -n "$$running" ]; then $(COMPOSE) start $(SERVICE); fi; \
	exit $$status

# University of Utah CS Discord Bot

This is a bot for the University of Utah CS Discord, written in Rust.

## Contributions

Feel free to open a PR with any changes, either taking something from issues or doing something yourself!

You can also open an issue if you have a suggestion or bug report.

## First time setup

You'll need rust installed. You can install it from https://rustup.rs/.

If you haven't learned rust, you'll need to. [Learn from the book](https://doc.rust-lang.org/book/).

Clone this repo and `cd` into it.

```bash
git clone git@github.com:coravacav/uofu-cs-discord-bot.git
cd uofu-cs-discord-bot
```

You'll need to create a bot using discord's developer portal. You can do this by going to https://discord.com/developers/applications and clicking "New Application".

Then, put the token in `.env` as `DISCORD_TOKEN` at the root of the project. For example:

```env
DISCORD_TOKEN="your token"
```

Next, for whatever server you'll run the bot in, you'll want to list the server id in the `config.toml` file. You can find the server id by right clicking on the server name in discord and clicking "Copy ID".

Then put it under `guild_id` in the config file.

Finally, run `cargo run` to start the bot.

You'll see an error about missing the LLM, but, that's okay. The command just won't work.



## Running in Docker (production)

The bot runs as the `kingfisher` compose project (container `kingfisher-bot`). The repo checkout is
bind-mounted at `/app` as the bot's working directory, so `.env`, `config.toml` (hot-reloaded),
`db/kingfisher-v3`, `debug.json` and `extracts/` stay in the repo dir and are never baked into the image.
The container restarts automatically (`restart: unless-stopped`), including after Docker Desktop starts at login.

```sh
just deploy     # rebuild the image and restart the bot (the update path after `git pull`)
just build      # build only (cargo deps cached, rebuilds are incremental)
just up         # start without rebuilding
just down       # stop and remove the container
just restart    # restart the container
just logs       # follow logs
just status     # container state + restart count
just shell      # shell inside the container
just backup-db  # stop briefly, copy db/kingfisher-v3 to backups/kingfisher-v3-<timestamp>, start again
```

Never run `cargo run`/`target/release/bot` against `db/kingfisher-v3` while the container is up: the RocksDB
lock does not protect across the Docker VM boundary. The stack shows up in Portainer as an external stack
(`kingfisher`); since the image is built locally, update it with `just deploy`, not Portainer's redeploy.

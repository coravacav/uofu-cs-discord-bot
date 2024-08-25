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



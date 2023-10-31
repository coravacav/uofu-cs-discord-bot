# University of Utah CS Discord Bot

This is a bot for the University of Utah CS Discord, written in Rust.

## Configuration
The bot will read from a `config.toml` file in the root directory.
If the file doesn't exist, it will create one, but bear in mind that
it will not run without either a valid bot token in the `DISCORD_TOKEN` environment variable,
or the `discord_token` field in the config file.

An example `config.toml` file looks like this:
```toml
text_detect_cooldown = 5
discord_token = "Your token here"

[[responses]]
name = "example"
ruleset = """
r ex(ample)?
r! not example
"""
content = "Hello, world!"
```
Note that the text detection cooldown is
specified in minutes.

Message responses are also specified in the configuration file,
in a `[[responses]]` array. The four possible types of response
are currently `Text`, `RandomText`, `Image`, and `TextAndImage`.
Fields that are common to all response types are the `name` and `pattern`
fields, while `content` is used for text content (an array of text in the case of `RandomText`),
and `path` is used for image content. The response type is inferred by which fields you have
and how they are used.
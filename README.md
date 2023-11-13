# University of Utah CS Discord Bot

This is a bot for the University of Utah CS Discord, written in Rust.

## Configuration

The bot will read from a `config.toml` file in the root directory.

Explanation of keys:

```toml
text_detect_cooldown = 45 # global response cooldown in seconds
starboard_reaction_count = 5 # how many reactions it takes to get to the starboard
starboard_emote_name = "face with raised eyebrow" # what emote to use for starboard reactions

[[responses]]
name = "rust" # some identifier, actually unused now but eh, do it anyway
# ruleset is a custom language
# see examples, but, basically, it's a list of regexes
# and if all of them match (separated by `or`) then the response is triggered
# in this case, if the message contains the word "rust" (case insensitive)
ruleset = """
r (?i)rust
"""
# content is a list of strings to choose at random
# could also be a single string
content = [
    "RUST MENTIONED :crab: :crab: :crab:",
    "<@216767618923757568>",
    "Rust is simply the best programming language. Nothing else can compare. I am namingmy kids Rust and Ferris.",
    """
Launch the Polaris,
the end doesn't scare us
When will this cease?
The warheads will all rust in peace!""",
    "Rust? Oh, you mean the game?",
]
```

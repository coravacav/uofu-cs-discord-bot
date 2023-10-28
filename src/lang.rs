use crate::memory_regex::MemoryRegex;
use nom::{
    bytes::complete::tag,
    bytes::complete::take_till,
    character::complete::{char, newline},
    combinator::{map_res, opt, value},
    multi::many1,
    sequence::{terminated, tuple},
    IResult,
};
use regex::Error;
use serde::{de::Visitor, Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    RegexUnparsed,
    Regex(MemoryRegex),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub kind: Kind,
    pub negated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ruleset {
    lines: Vec<Line>,
}

impl Ruleset {
    pub fn new(lines: Vec<Line>) -> Self {
        Self { lines }
    }

    pub fn matches(&self, input: &str) -> bool {
        self.lines.iter().all(|line| {
            let res = match &line.kind {
                Kind::RegexUnparsed => panic!("unparsed regex"),
                Kind::Regex(regex) => regex.is_match(input),
            };

            if line.negated {
                !res
            } else {
                res
            }
        })
    }
}

impl std::ops::Deref for Ruleset {
    type Target = Vec<Line>;

    fn deref(&self) -> &Self::Target {
        &self.lines
    }
}

impl std::ops::DerefMut for Ruleset {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.lines
    }
}

struct RulesetVisitor {}

impl RulesetVisitor {
    fn visit_str<E>(self, v: &str) -> Result<Ruleset, E>
    where
        E: serde::de::Error,
    {
        if let Some(v) = parse(v) {
            Ok(Ruleset { lines: v })
        } else {
            Err(E::custom("invalid ruleset"))
        }
    }
}

impl<'de> Visitor<'de> for RulesetVisitor {
    type Value = Ruleset;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a valid ruleset")
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(v)
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&v)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&v)
    }
}

impl<'de> Deserialize<'de> for Ruleset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(RulesetVisitor {})
    }
}

impl Serialize for Ruleset {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        let mut s = String::new();
        for line in &self.lines {
            if line.negated {
                s.push('!');
            }

            match &line.kind {
                Kind::RegexUnparsed => panic!("unparsed regex"),
                Kind::Regex(lazy) => {
                    s.push_str(&format!("r {}", lazy.as_str()));
                }
            };

            s.push('\n');
        }

        serializer.serialize_str(&s)
    }
}

fn parser(input: &str) -> IResult<&str, Vec<Line>> {
    many1(parse_line)(input)
}

fn parse_line(input: &str) -> IResult<&str, Line> {
    map_res(
        terminated(
            tuple((
                opt(tag("!")),
                terminated(value(Kind::RegexUnparsed, tag("r")), char(' ')),
                take_till(|c| c == '\n'),
            )),
            newline,
        ),
        |(negated, kind, content)| {
            let negated: Option<&str> = negated; // Needs to be coerced for some reason.

            Ok::<Line, Error>(Line {
                negated: negated.is_some(),
                kind: match kind {
                    Kind::RegexUnparsed => Kind::Regex(MemoryRegex::new(content.to_string())?),
                    Kind::Regex(_) => kind,
                },
            })
        },
    )(input)
}

pub fn parse(input: &str) -> Option<Vec<Line>> {
    parser(input.trim_start()).map(|v| v.1).ok()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_deserialize() {
        let test = r#"
r 1234
!r 4321
"#;

        let result = parse(test).unwrap();
        assert_eq!(
            result,
            vec![
                Line {
                    kind: Kind::Regex(MemoryRegex::new("1234".to_string()).unwrap()),
                    negated: false,
                },
                Line {
                    kind: Kind::Regex(MemoryRegex::new("4321".to_string()).unwrap()),
                    negated: true,
                },
            ]
        );
    }

    #[test]
    fn test_serialize() {
        let ruleset = Ruleset::new(vec![
            Line {
                kind: Kind::Regex(MemoryRegex::new("1234".to_string()).unwrap()),
                negated: false,
            },
            Line {
                kind: Kind::Regex(MemoryRegex::new("4321".to_string()).unwrap()),
                negated: true,
            },
        ]);

        #[derive(Serialize)]
        struct RulesetContainer {
            ruleset: Ruleset,
        }

        let result = toml::to_string(&RulesetContainer { ruleset }).unwrap();

        assert_eq!(
            result,
            r#"ruleset = """
r 1234
!r 4321
"""
"#
        );
    }
}

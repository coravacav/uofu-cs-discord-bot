use crate::memory_regex::MemoryRegex;
use nom::{
    bytes::complete::tag,
    bytes::complete::take_till,
    character::complete::{char, newline},
    combinator::{map_res, opt, value},
    multi::separated_list1,
    sequence::{terminated, tuple},
    IResult,
};
use regex::Error;

mod serialization;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    RegexUnparsed,
    Regex(MemoryRegex),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    pub kind: Kind,
    pub negated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    cases: Vec<Case>,
}

impl Rule {
    pub fn new(case: Vec<Case>) -> Self {
        Self { cases: case }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ruleset {
    rules: Vec<Rule>,
}

impl Ruleset {
    #[allow(dead_code)] // We use this at least in tests.
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    pub fn matches(&self, input: &str) -> bool {
        self.rules.iter().any(|rule| {
            rule.cases.iter().all(|case| {
                let res = match &case.kind {
                    Kind::RegexUnparsed => panic!("unparsed regex"),
                    Kind::Regex(regex) => regex.is_match(input),
                };

                if case.negated {
                    !res
                } else {
                    res
                }
            })
        })
    }
}

impl std::ops::Deref for Ruleset {
    type Target = Vec<Rule>;

    fn deref(&self) -> &Self::Target {
        &self.rules
    }
}

impl std::ops::DerefMut for Ruleset {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.rules
    }
}

fn parse_case(input: &str) -> IResult<&str, Case> {
    map_res(
        tuple((
            opt(tag("!")),
            terminated(value(Kind::RegexUnparsed, tag("r")), char(' ')),
            take_till(|c| c == '\n'),
        )),
        |(negated, kind, content)| {
            let negated: Option<&str> = negated; // Needs to be coerced for some reason.

            Ok::<Case, Error>(Case {
                negated: negated.is_some(),
                kind: match kind {
                    Kind::RegexUnparsed => Kind::Regex(MemoryRegex::new(content.to_string())?),
                    Kind::Regex(_) => kind,
                },
            })
        },
    )(input)
}

fn parse_all_cases(input: &str) -> IResult<&str, Vec<Case>> {
    separated_list1(newline, parse_case)(input)
}

fn parse_rules(input: &str) -> Option<Vec<Rule>> {
    match separated_list1(tag(SPLIT_RULE_SEPARATOR), parse_all_cases)(input) {
        Ok((_, vec_vec_case)) => Some(vec_vec_case.into_iter().map(Rule::new).collect()),
        Err(e) => {
            eprintln!("Error parsing rules: {:?}", e);
            None
        }
    }
}

pub fn parse(input: &str) -> Option<Vec<Rule>> {
    parse_rules(input.trim_start())
}

#[macro_export]
macro_rules! fast_ruleset {
    ($($x:expr),*) => {{
        $crate::lang::parse(&[$($x),*].join("\n")).map(Ruleset::new).unwrap()
    }};
}

const SPLIT_RULE_SEPARATOR: &str = "\nor\n";

#[cfg(test)]
mod test {
    use serde::Serialize;

    use super::*;

    #[test]
    fn test_detection() {
        let ruleset = fast_ruleset!("r 1234", "or", "r :3", "or", "r mew");

        assert!(ruleset.matches("mew"));
        assert!(ruleset.matches(":3"));
        assert!(ruleset.matches("1234"));
        assert!(!ruleset.matches("123"));
    }

    #[test]
    fn test_detection_2() {
        let ruleset = fast_ruleset!(
            r"r (?i)\bme+o*w\b",
            "or",
            "r (?i)[ou]w[ou]",
            "or",
            "r 喵",
            "or",
            "r :3",
            "or",
            r"r (?i)\bee+p.*",
            "or",
            "r (?i)ny+a+",
            "or",
            "r (?i)mrr+[pb]",
            "or",
            "r (?i)pu+rr+"
        );

        assert!(ruleset.matches("mew"));
        assert!(ruleset.matches(":3"));
        assert!(!ruleset.matches("123"));
        assert!(ruleset.matches("meow"));
    }

    #[test]
    fn test_parse_case() {
        let test = "!r 1234";

        let result = parse_case(test).unwrap();
        assert_eq!(
            result,
            (
                "",
                Case {
                    kind: Kind::Regex(MemoryRegex::new("1234".to_string()).unwrap()),
                    negated: true,
                }
            )
        );
    }

    #[test]
    fn test_parse_all_cases() {
        let test = "!r 1234\nr 4321";

        let result = parse_all_cases(test).unwrap();
        assert_eq!(
            result,
            (
                "",
                vec![
                    Case {
                        kind: Kind::Regex(MemoryRegex::new("1234".to_string()).unwrap()),
                        negated: true,
                    },
                    Case {
                        kind: Kind::Regex(MemoryRegex::new("4321".to_string()).unwrap()),
                        negated: false,
                    },
                ]
            )
        );
    }

    #[test]
    fn test_deserialize() {
        let ruleset = fast_ruleset!("r 1234", "!r 4321", "or", "r 3333");

        assert_eq!(
            ruleset,
            Ruleset::new(vec![
                Rule {
                    cases: vec![
                        Case {
                            kind: Kind::Regex(MemoryRegex::new("1234".to_string()).unwrap()),
                            negated: false,
                        },
                        Case {
                            kind: Kind::Regex(MemoryRegex::new("4321".to_string()).unwrap()),
                            negated: true,
                        },
                    ],
                },
                Rule {
                    cases: vec![Case {
                        kind: Kind::Regex(MemoryRegex::new("3333".to_string()).unwrap()),
                        negated: false,
                    }],
                },
            ])
        );
    }

    #[test]
    fn test_serialize() {
        let ruleset = Ruleset::new(vec![
            Rule {
                cases: vec![
                    Case {
                        kind: Kind::Regex(MemoryRegex::new("1234".to_string()).unwrap()),
                        negated: false,
                    },
                    Case {
                        kind: Kind::Regex(MemoryRegex::new("4321".to_string()).unwrap()),
                        negated: true,
                    },
                ],
            },
            Rule {
                cases: vec![Case {
                    kind: Kind::Regex(MemoryRegex::new("3333".to_string()).unwrap()),
                    negated: false,
                }],
            },
            Rule {
                cases: vec![Case {
                    kind: Kind::Regex(MemoryRegex::new("6969".to_string()).unwrap()),
                    negated: true,
                }],
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
or
r 3333
or
!r 6969
"""
"#
        );
    }
}

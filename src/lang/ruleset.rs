use super::rule::{parse_rules, Rule};
use serde::{de::Visitor, Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Ruleset {
    rules: Vec<Rule>,
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

impl Ruleset {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    pub fn parse(input: &str) -> Option<Self> {
        parse_rules(input.trim_start()).map(Self::new)
    }

    pub fn matches(&self, input: &str) -> bool {
        self.rules.iter().any(|rule| {
            rule.cases.iter().all(|case| {
                let res = case.regex.is_match(input);

                if case.negated {
                    !res
                } else {
                    res
                }
            })
        })
    }
}

struct RulesetVisitor {}

impl RulesetVisitor {
    fn visit_str<E>(self, v: &str) -> Result<Ruleset, E>
    where
        E: serde::de::Error,
    {
        if let Some(ruleset) = Ruleset::parse(v) {
            Ok(ruleset)
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
        self.visit_str(v)
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
        for rule in &self.rules {
            for case in &rule.cases {
                if case.negated {
                    s.push('!');
                }

                s.push_str(&format!("r {}\n", case.regex.as_str()));
            }

            s.push_str("or\n");
        }

        // Remove the final next
        // This way is funnier than the better solution
        s.pop();
        s.pop();
        s.pop();

        serializer.serialize_str(&s)
    }
}

#[cfg(test)]
mod test {
    use regex::Regex;

    use super::*;
    use crate::{fast_ruleset, lang::case::Case};

    #[test]
    fn test_deserialize() {
        let ruleset = fast_ruleset!("r 1234", "!r 4321", "or", "r 3333");

        assert_eq!(
            ruleset,
            Ruleset::new(vec![
                Rule {
                    cases: vec![
                        Case {
                            regex: Regex::new("1234").unwrap(),
                            negated: false,
                        },
                        Case {
                            regex: Regex::new("4321").unwrap(),
                            negated: true,
                        },
                    ],
                },
                Rule {
                    cases: vec![Case {
                        regex: Regex::new("3333").unwrap(),
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
                        regex: Regex::new("1234").unwrap(),
                        negated: false,
                    },
                    Case {
                        regex: Regex::new("4321").unwrap(),
                        negated: true,
                    },
                ],
            },
            Rule {
                cases: vec![Case {
                    regex: Regex::new("3333").unwrap(),
                    negated: false,
                }],
            },
            Rule {
                cases: vec![Case {
                    regex: Regex::new("6969").unwrap(),
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

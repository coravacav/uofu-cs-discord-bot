use super::rule::{parse_rules, Rule};
use color_eyre::eyre::{Context, ContextCompat, Result};
use regex::Regex;
use serde::{de::Visitor, Deserialize, Deserializer};

struct UnparsedRegexAndNegated<'a>(&'a str, bool);

#[derive(Clone, Debug)]
struct RegexAndNegated(Regex, bool);

#[derive(Clone, Debug)]
struct RegexPair {
    positive: Option<Regex>,
    negative: Option<Regex>,
}

impl RegexPair {
    fn has_any(&self) -> bool {
        self.positive.is_some() || self.negative.is_some()
    }
}

#[derive(Clone, Debug)]
enum RegexRules {
    Single(RegexPair),
    Multiple(Vec<RegexAndNegated>),
}

#[derive(Clone, Debug, Default)]
pub struct Ruleset {
    rules: Vec<RegexRules>,
}

impl<'a> Ruleset {
    pub fn new(rules: Vec<Rule<'a>>) -> Result<Self> {
        let mut single_rules = vec![];

        let mut completed_rules: Vec<RegexRules> = vec![];

        for rule in rules {
            match rule.cases.len() {
                1 => single_rules.push(UnparsedRegexAndNegated(
                    rule.cases[0].unparsed_regex,
                    rule.cases[0].negated,
                )),
                2.. => {
                    completed_rules.push(RegexRules::Multiple(
                        rule.cases
                            .into_iter()
                            .map(|case| {
                                Ok(RegexAndNegated(
                                    Regex::new(case.unparsed_regex)
                                        .wrap_err("Regex failed to compile")?,
                                    case.negated,
                                ))
                            })
                            .collect::<Result<Vec<RegexAndNegated>>>()?,
                    ));
                }
                _ => unreachable!(),
            }
        }

        let single_rule = Self::combine_regexes(single_rules)?;
        if single_rule.has_any() {
            completed_rules.push(RegexRules::Single(single_rule));
        }

        Ok(Self {
            rules: completed_rules,
        })
    }

    fn combine_regexes(unparsed_regexes: Vec<UnparsedRegexAndNegated>) -> Result<RegexPair> {
        let mut efficient_regex_string_positive = String::new();
        let mut efficient_regex_string_negative = String::new();

        for UnparsedRegexAndNegated(rule, negated) in unparsed_regexes {
            let regex_string = if negated {
                &mut efficient_regex_string_negative
            } else {
                &mut efficient_regex_string_positive
            };

            regex_string.push_str("(?:");
            regex_string.push_str(rule);
            regex_string.push_str(")|");
        }

        let mut positive_regex = None;
        let mut negative_regex = None;

        if !efficient_regex_string_positive.is_empty() {
            efficient_regex_string_positive.pop();
            positive_regex = Some(
                Regex::new(&efficient_regex_string_positive)
                    .wrap_err("Couldn't compile positive regex")?,
            );
        }

        if !efficient_regex_string_negative.is_empty() {
            efficient_regex_string_negative.pop();
            negative_regex = Some(
                Regex::new(&efficient_regex_string_negative)
                    .wrap_err("Couldn't compile negative regex")?,
            );
        }

        Ok(RegexPair {
            positive: positive_regex,
            negative: negative_regex,
        })
    }

    pub fn parse(input: &'a str) -> Result<Self> {
        Self::new(parse_rules(input.trim_start()).wrap_err("Couldn't parse rules")?)
    }

    pub fn matches(&self, input: &str) -> bool {
        self.rules.iter().any(|rule| match rule {
            RegexRules::Single(RegexPair { negative, positive }) => match (negative, positive) {
                (Some(negative), Some(positive)) => {
                    !negative.is_match(input) && positive.is_match(input)
                }
                (Some(negative), None) => !negative.is_match(input),
                (None, Some(positive)) => positive.is_match(input),
                (None, None) => false,
            },
            RegexRules::Multiple(regexes) => {
                regexes.iter().all(|RegexAndNegated(regex, negated)| {
                    let res = regex.is_match(input);
                    if *negated {
                        !res
                    } else {
                        res
                    }
                })
            }
        })
    }
}

struct RulesetVisitor {}

impl RulesetVisitor {
    fn visit_str<E>(self, v: &str) -> Result<Ruleset, E>
    where
        E: serde::de::Error,
    {
        match Ruleset::parse(v) {
            Ok(ruleset) => Ok(ruleset),
            Err(e) => Err(E::custom(format!("invalid ruleset: {}", e))),
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

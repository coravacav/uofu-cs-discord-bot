use super::{
    rule::{Rule, parse_rules},
    ruleset_combinator::create_matcher_regex,
};
use color_eyre::eyre::{ContextCompat, Result, bail};
use regex::{Regex, RegexSet};

#[derive(Clone)]
pub struct UnparsedRegexAndNegated<'a>(pub &'a str, pub bool);

#[derive(Clone)]
pub enum UnparsedRegex<'a> {
    Single(&'a str),
    Multiple(Vec<UnparsedRegexAndNegated<'a>>),
}

#[derive(Clone)]
pub struct UnparsedRuleset<'a> {
    pub regexes: Vec<UnparsedRegex<'a>>,
}

#[derive(Clone)]
pub struct RegexAndNegated(pub Regex, pub bool);

pub struct Ruleset {
    /// If this matches, the rule is considered a match
    pub single_positive: Option<RegexSet>,
    /// If all of these match, the rule is considered a match
    pub multiple: Option<Vec<Vec<RegexAndNegated>>>,
}

impl<'a> UnparsedRuleset<'a> {
    pub fn new(rules: Vec<Rule<'a>>) -> Result<Self> {
        let mut completed_rules: Vec<UnparsedRegex<'a>> = vec![];

        for rule in rules {
            match rule.cases.len() {
                1 => {
                    if rule.cases[0].negated {
                        bail!("Negative standalone rules are not supported");
                    }

                    completed_rules.push(UnparsedRegex::Single(rule.cases[0].unparsed_regex))
                }
                2.. => {
                    completed_rules.push(UnparsedRegex::Multiple(
                        rule.cases
                            .into_iter()
                            .map(|case| UnparsedRegexAndNegated(case.unparsed_regex, case.negated))
                            .collect(),
                    ));
                }
                _ => unreachable!(),
            }
        }

        Ok(Self {
            regexes: completed_rules,
        })
    }

    pub fn parse(input: &'a str) -> Result<Self> {
        Self::new(parse_rules(input.trim_start()).wrap_err("Couldn't parse rules")?)
    }
}

impl<'a> TryFrom<&'a str> for UnparsedRuleset<'a> {
    type Error = color_eyre::eyre::Error;

    fn try_from(value: &'a str) -> Result<Self> {
        Self::parse(value)
    }
}

impl TryFrom<UnparsedRuleset<'_>> for Ruleset {
    type Error = color_eyre::eyre::Error;

    fn try_from(unparsed_ruleset: UnparsedRuleset) -> Result<Self> {
        let mut current_ruleset_positive = vec![];
        let mut multiple: Vec<Vec<RegexAndNegated>> = vec![];

        for unparsed_regex in unparsed_ruleset.regexes {
            match unparsed_regex {
                UnparsedRegex::Single(unparsed_regex) => {
                    current_ruleset_positive.push(unparsed_regex);
                }
                UnparsedRegex::Multiple(vec) => {
                    let mut this_multiple = vec![];

                    for UnparsedRegexAndNegated(unparsed_regex, negated) in vec {
                        this_multiple.push(RegexAndNegated(Regex::new(unparsed_regex)?, negated));
                    }

                    multiple.push(this_multiple);
                }
            };
        }

        let single_positive = create_matcher_regex(&current_ruleset_positive)?;

        let multiple = if multiple.is_empty() {
            None
        } else {
            Some(multiple)
        };

        Ok(Self::new(single_positive, multiple))
    }
}

impl Ruleset {
    pub fn new(
        single_positive: Option<RegexSet>,
        multiple: Option<Vec<Vec<RegexAndNegated>>>,
    ) -> Self {
        Self {
            single_positive,
            multiple,
        }
    }

    pub fn matches(&self, input: &str) -> bool {
        if let Some(positive) = &self.single_positive
            && positive.is_match(input)
        {
            return true;
        }

        if let Some(multi_rules) = &self.multiple {
            return multi_rules.iter().any(|multi_rule| {
                multi_rule.iter().all(|RegexAndNegated(regex, negated)| {
                    let res = regex.is_match(input);
                    if *negated { !res } else { res }
                })
            });
        }

        false
    }
}

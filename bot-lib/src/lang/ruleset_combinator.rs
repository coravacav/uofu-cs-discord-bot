use std::sync::Arc;

use ahash::{AHashMap, AHashSet};
use color_eyre::eyre::{bail, Result};
use regex::Regex;

use super::ruleset::{
    RegexAndNegated, Ruleset, UnparsedRegex, UnparsedRegexAndNegated, UnparsedRuleset,
};

type Name = Arc<str>;

pub struct UnparsedRulesetWithName<'a> {
    pub name: Name,
    pub unparsed_ruleset: UnparsedRuleset<'a>,
}

impl<'a> From<(Name, UnparsedRuleset<'a>)> for UnparsedRulesetWithName<'a> {
    fn from((name, unparsed_ruleset): (Name, UnparsedRuleset<'a>)) -> Self {
        Self {
            name,
            unparsed_ruleset,
        }
    }
}

#[derive(Debug)]
pub struct RulesetCombinator {
    /// If this matches true, at least one of the rules is considered a match.
    single_positive_matcher: Option<Regex>,
    /// If this matches false, at least one of the rules is considered a match.
    single_negative_matcher: Option<Regex>,
    /// If single_positive_matcher, one of these rules is considered a match.
    single_positive_rulesets: Vec<Name>,
    /// If single_negative_matcher, one of these rules is considered a match.
    single_negative_rulesets: Vec<Name>,
    /// If this matches true, at least one of the rules is considered a match.
    multiple_positive_matcher: Option<Regex>,
    /// If this matches false, at least one of the rules is considered a match.
    multiple_negative_matcher: Option<Regex>,
    multiple_positive_rulesets: Vec<Name>,
    multiple_negative_rulesets: Vec<Name>,
    multiple_rulesets: Vec<Name>,
    rulesets: AHashMap<Name, Ruleset>,
}

impl RulesetCombinator {
    pub fn new<'a>(
        unparsed_rulesets: impl Iterator<Item = UnparsedRulesetWithName<'a>>,
    ) -> Result<Self> {
        let mut single_positive_options: Vec<&str> = vec![];
        let mut single_negative_options: Vec<&str> = vec![];
        let mut single_positive_rulesets: AHashSet<Name> = AHashSet::new();
        let mut single_negative_rulesets: AHashSet<Name> = AHashSet::new();
        let mut multiple_positive_rulesets: AHashSet<Name> = AHashSet::new();
        let mut multiple_negative_rulesets: AHashSet<Name> = AHashSet::new();
        let mut multiple_rulesets: AHashSet<Name> = AHashSet::new();
        let mut rulesets: AHashMap<Name, Ruleset> = AHashMap::new();

        let mut multiple_positive_options: Vec<&str> = vec![];
        let mut multiple_negative_options: Vec<&str> = vec![];

        for UnparsedRulesetWithName {
            name,
            unparsed_ruleset,
        } in unparsed_rulesets
        {
            let mut single_positive = vec![];
            let mut single_negative = vec![];
            let mut multiple: Vec<Vec<RegexAndNegated>> = vec![];

            for unparsed_regex in unparsed_ruleset.regexes {
                match unparsed_regex {
                    UnparsedRegex::Single(UnparsedRegexAndNegated(unparsed_regex, negated)) => {
                        if negated {
                            single_negative_options.push(unparsed_regex);
                            single_negative.push(unparsed_regex);
                            single_negative_rulesets.insert(name.clone());
                        } else {
                            single_positive_options.push(unparsed_regex);
                            single_positive.push(unparsed_regex);
                            single_positive_rulesets.insert(name.clone());
                        }
                    }
                    UnparsedRegex::Multiple(vec) => {
                        let mut has_positive = false;
                        let mut has_negative = false;

                        multiple_rulesets.insert(name.clone());

                        let mut new_multiple = vec![];

                        for UnparsedRegexAndNegated(unparsed_regex, negated) in vec {
                            if negated {
                                has_negative = true;
                                multiple_negative_options.push(unparsed_regex);
                            } else {
                                has_positive = true;
                                multiple_positive_options.push(unparsed_regex);
                            }

                            new_multiple
                                .push(RegexAndNegated(Regex::new(unparsed_regex)?, negated));
                        }

                        if has_positive {
                            multiple_positive_rulesets.insert(name.clone());
                        }

                        if has_negative {
                            multiple_negative_rulesets.insert(name.clone());
                        }

                        multiple.push(new_multiple);
                    }
                };
            }

            let single_positive = create_matcher_regex(&single_positive)?;
            let single_negative = create_matcher_regex(&single_negative)?;

            let multiple = if multiple.is_empty() {
                None
            } else {
                Some(multiple)
            };

            if rulesets
                .insert(
                    name.clone(),
                    Ruleset::new(single_positive, single_negative, multiple),
                )
                .is_some()
            {
                bail!("Duplicate ruleset name: {}", name);
            }
        }

        let single_positive_matcher = create_matcher_regex(&single_positive_options)?;
        let single_negative_matcher = create_matcher_regex(&single_negative_options)?;
        let multiple_positive_matcher = create_matcher_regex(&multiple_positive_options)?;
        let multiple_negative_matcher = create_matcher_regex(&multiple_negative_options)?;

        Ok(Self {
            single_positive_matcher,
            single_negative_matcher,
            multiple_negative_matcher,
            multiple_positive_matcher,
            rulesets,
            single_positive_rulesets: single_positive_rulesets.into_iter().collect(),
            single_negative_rulesets: single_negative_rulesets.into_iter().collect(),
            multiple_positive_rulesets: multiple_positive_rulesets.into_iter().collect(),
            multiple_negative_rulesets: multiple_negative_rulesets.into_iter().collect(),
            multiple_rulesets: multiple_rulesets.into_iter().collect(),
        })
    }

    pub fn matches(&self, input: &str) -> bool {
        if let Some(positive) = &self.single_positive_matcher {
            if positive.is_match(input) {
                return true;
            }
        }

        if let Some(negative) = &self.single_negative_matcher {
            if !negative.is_match(input) {
                return true;
            }
        }

        if self.multiple_positive_matcher.is_some() {
            for multi_rule in &self.multiple_rulesets {
                if self.rulesets[multi_rule].matches(input) {
                    return true;
                }
            }
        }

        false
    }

    pub fn find_iter<'a>(&'a self, input: &'a str) -> impl Iterator<Item = Name> + use<'a> {
        let name_leads_to_match = |name: &Name| {
            self.rulesets
                .get(name)
                .filter(|ruleset| ruleset.matches(input))
                .map(|_| name.clone())
        };

        let positive_iter = if let Some(positive) = &self.single_positive_matcher {
            if positive.is_match(input) {
                Some(
                    self.single_positive_rulesets
                        .iter()
                        .filter_map(name_leads_to_match),
                )
            } else {
                None
            }
        } else {
            None
        };

        let negative_iter = if let Some(negative) = &self.single_negative_matcher {
            if !negative.is_match(input) {
                Some(
                    self.single_negative_rulesets
                        .iter()
                        .filter_map(name_leads_to_match),
                )
            } else {
                None
            }
        } else {
            None
        };

        let multiple_iter = match (
            &self.multiple_positive_matcher,
            &self.multiple_negative_matcher,
        ) {
            (Some(positive), Some(negative)) => {
                (positive.is_match(input) || !negative.is_match(input)).then(|| {
                    self.multiple_rulesets
                        .iter()
                        .filter_map(name_leads_to_match)
                })
            }
            (Some(positive), None) => positive.is_match(input).then(|| {
                self.multiple_positive_rulesets
                    .iter()
                    .filter_map(name_leads_to_match)
            }),
            (None, Some(negative)) => (!negative.is_match(input)).then(|| {
                self.multiple_negative_rulesets
                    .iter()
                    .filter_map(name_leads_to_match)
            }),
            (None, None) => None,
        };

        positive_iter
            .into_iter()
            .flatten()
            .chain(negative_iter.into_iter().flatten())
            .chain(multiple_iter.into_iter().flatten())
    }
}

pub fn create_matcher_regex(options: &[&str]) -> Result<Option<Regex>> {
    if options.is_empty() {
        return Ok(None);
    }

    let mut regex_string = String::new();

    for option in options {
        regex_string.push_str("(?:");
        regex_string.push_str(option);
        regex_string.push_str(")|");
    }

    regex_string.pop();

    Ok(Some(Regex::new(&regex_string)?))
}

//! This module is responsible for efficiently constructing and evaluating multiple rulesets.
//!
//! The design is based on the idea that most messages don't match any rulesets,
//! so, if we can quickly check

use std::sync::Arc;

use color_eyre::eyre::{Result, bail};
use regex::{Regex, RegexSet};
use rustc_hash::FxHashMap;

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

pub struct RulesetCombinator {
    /// If this matches true, at least one of the rules is considered a match.
    single_positive_matcher: Option<RegexSet>,
    /// If this matches true, at least one of the rules is considered a match.
    multiple_positive_matcher: Option<RegexSet>,
    /// If this matches false, at least one of the rules is considered a match.
    multiple_negative_matcher: Option<RegexSet>,
    /// If single_positive_matcher, one of these rules is considered a match.
    single_positive_rulesets: Vec<Name>,
    multiple_positive_rulesets: Vec<Name>,
    multiple_negative_rulesets: Vec<Name>,
    rulesets: FxHashMap<Name, Ruleset>,
}

impl RulesetCombinator {
    pub fn new<'a>(
        unparsed_rulesets: impl Iterator<Item = UnparsedRulesetWithName<'a>>,
    ) -> Result<Self> {
        let mut rulesets: FxHashMap<Name, Ruleset> = FxHashMap::default();

        let mut single_positive_options: Vec<&str> = vec![];
        let mut multiple_positive_options: Vec<&str> = vec![];
        let mut multiple_negative_options: Vec<&str> = vec![];

        let mut single_positive_rulesets: Vec<Name> = vec![];
        let mut multiple_positive_rulesets: Vec<Name> = vec![];
        let mut multiple_negative_rulesets: Vec<Name> = vec![];

        for UnparsedRulesetWithName {
            name,
            unparsed_ruleset,
        } in unparsed_rulesets
        {
            let mut single_positive = vec![];
            let mut multiple: Vec<Vec<RegexAndNegated>> = vec![];

            for unparsed_regex in unparsed_ruleset.regexes {
                match unparsed_regex {
                    UnparsedRegex::Single(unparsed_regex) => {
                        single_positive_options.push(unparsed_regex);
                        single_positive.push(unparsed_regex);
                        single_positive_rulesets.push(name.clone());
                    }
                    UnparsedRegex::Multiple(vec) => {
                        let mut new_multiple = vec![];

                        for UnparsedRegexAndNegated(unparsed_regex, negated) in vec {
                            if negated {
                                multiple_negative_options.push(unparsed_regex);
                                multiple_negative_rulesets.push(name.clone());
                            } else {
                                multiple_positive_options.push(unparsed_regex);
                                multiple_positive_rulesets.push(name.clone());
                            }

                            new_multiple
                                .push(RegexAndNegated(Regex::new(unparsed_regex)?, negated));
                        }

                        multiple.push(new_multiple);
                    }
                };
            }

            let single_positive = create_matcher_regex(&single_positive)?;

            let multiple = if multiple.is_empty() {
                None
            } else {
                Some(multiple)
            };

            if rulesets
                .insert(name.clone(), Ruleset::new(single_positive, multiple))
                .is_some()
            {
                bail!("Duplicate ruleset name: {}", name);
            }
        }

        let single_positive_matcher = create_matcher_regex(&single_positive_options)?;
        let multiple_positive_matcher = create_matcher_regex(&multiple_positive_options)?;
        let multiple_negative_matcher = create_matcher_regex(&multiple_negative_options)?;

        Ok(Self {
            single_positive_matcher,
            multiple_negative_matcher,
            multiple_positive_matcher,
            rulesets,
            single_positive_rulesets,
            multiple_positive_rulesets,
            multiple_negative_rulesets,
        })
    }

    pub fn matches(&self, input: &str) -> bool {
        self.find_iter(input).next().is_some()
    }

    pub fn find_iter<'a>(&'a self, input: &'a str) -> impl Iterator<Item = Name> {
        let positive_iter = self.single_positive_matcher.as_ref().and_then(|positive| {
            positive
                .matches(input)
                .iter()
                .next()
                .map(|idx| self.single_positive_rulesets[idx].clone())
        });

        let multiple_positive_iter = self
            .multiple_positive_matcher
            .as_ref()
            .and_then(|positive| {
                positive
                    .matches(input)
                    .iter()
                    .map(|idx| self.multiple_positive_rulesets[idx].clone())
                    .find(|name| {
                        self.rulesets
                            .get(name)
                            .is_some_and(|ruleset| ruleset.matches(input))
                    })
            });

        let multiple_negative_iter = self
            .multiple_negative_matcher
            .as_ref()
            .and_then(|negative| {
                // this one is different, I need to get all the indexes of non matched
                let match_idxs = negative.matches(input);

                if match_idxs.matched_all() {
                    return None;
                }

                (0..negative.len())
                    .filter(|idx| !match_idxs.matched(*idx))
                    .map(|idx| self.multiple_negative_rulesets[idx].clone())
                    .find(|name| {
                        self.rulesets
                            .get(name)
                            .is_some_and(|ruleset| ruleset.matches(input))
                    })
            });

        positive_iter
            .into_iter()
            .chain(multiple_positive_iter)
            .chain(multiple_negative_iter)
    }
}

pub fn create_matcher_regex(options: &[&str]) -> Result<Option<RegexSet>> {
    if options.is_empty() {
        return Ok(None);
    }

    Ok(Some(RegexSet::new(options)?))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_create_matcher_regex() {
        let matcher = RulesetCombinator::new(
            vec![UnparsedRulesetWithName {
                name: "test".into(),
                unparsed_ruleset: UnparsedRuleset {
                    regexes: vec![UnparsedRegex::Multiple(vec![
                        UnparsedRegexAndNegated(r"goth", false),
                        UnparsedRegexAndNegated(r"woman", false),
                    ])],
                },
            }]
            .into_iter(),
        )
        .unwrap();

        assert!(matcher.matches("goth woman"));
        assert!(matcher.matches("woman goth"));
        assert!(!matcher.matches("woman"));
    }
}

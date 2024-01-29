use super::case::{parse_all_cases, Case};
use nom::{bytes::complete::tag, multi::separated_list1, Finish};

const SPLIT_RULE_SEPARATOR: &str = "\nor\n";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub cases: Vec<Case>,
}

impl Rule {
    pub fn new(case: Vec<Case>) -> Self {
        Self { cases: case }
    }
}

pub fn parse_rules(input: &str) -> Option<Vec<Rule>> {
    separated_list1(tag(SPLIT_RULE_SEPARATOR), parse_all_cases)(input)
        .finish()
        .map(|(_, vec_vec_case)| vec_vec_case.into_iter().map(Rule::new).collect())
        .ok()
}

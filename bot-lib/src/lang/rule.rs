use super::case::{parse_all_cases, Case};
use nom::{
    bytes::complete::tag, character::complete::multispace1, multi::separated_list1,
    sequence::tuple, Finish, IResult,
};

const SPLIT_RULE_SEPARATOR: &str = "\nor";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub cases: Vec<Case>,
}

impl Rule {
    pub fn new(case: Vec<Case>) -> Self {
        Self { cases: case }
    }
}

pub fn parse_separator(input: &str) -> IResult<&str, ()> {
    tuple((tag(SPLIT_RULE_SEPARATOR), multispace1))(input).map(|(v, _)| (v, ()))
}

pub fn parse_rules(input: &str) -> Option<Vec<Rule>> {
    separated_list1(parse_separator, parse_all_cases)(input)
        .finish()
        .map(|(_, vec_vec_case)| vec_vec_case.into_iter().map(Rule::new).collect())
        .ok()
}

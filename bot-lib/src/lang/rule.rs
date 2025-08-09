use super::case::{Case, parse_all_cases};
use bot_traits::ForwardRefToTracing;
use nom::{
    Finish, IResult, bytes::complete::tag, character::complete::multispace1,
    multi::separated_list1, sequence::tuple,
};

/// A rule is a single case of success for a given ruleset
#[derive(Clone, PartialEq, Eq)]
pub struct Rule<'a> {
    /// The cases of the rule
    pub cases: Vec<Case<'a>>,
}

impl<'a> Rule<'a> {
    pub fn new(case: Vec<Case<'a>>) -> Self {
        Self { cases: case }
    }
}

pub fn parse_separator(input: &str) -> IResult<&str, ()> {
    tuple((tag("\nor"), multispace1))(input).map(|(v, _)| (v, ()))
}

pub fn parse_rules(input: &str) -> Option<Vec<Rule<'_>>> {
    separated_list1(parse_separator, parse_all_cases)(input)
        .finish()
        .map(|(_, vec_vec_case)| vec_vec_case.into_iter().map(Rule::new).collect())
        .trace_err_ok()
}

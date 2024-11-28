use nom::{
    IResult, bytes::complete::tag, bytes::complete::take_till, character::complete::newline,
    combinator::opt, multi::separated_list1,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case<'a> {
    pub unparsed_regex: &'a str,
    pub negated: bool,
}

fn parse_line(input: &str) -> IResult<&str, Case> {
    let (input, is_negated) = opt(tag("!"))(input)?;
    let is_negated = is_negated.is_some();
    let (input, _) = tag("r ")(input)?;
    let (input, unparsed_regex) = take_till(|c| c == '\n')(input)?;
    Ok((input, Case {
        unparsed_regex,
        negated: is_negated,
    }))
}

fn parse_case(input: &str) -> IResult<&str, Case> {
    parse_line(input)
}

pub fn parse_all_cases(input: &str) -> IResult<&str, Vec<Case>> {
    separated_list1(newline, parse_case)(input)
}

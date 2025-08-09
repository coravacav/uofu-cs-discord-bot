use nom::{
    IResult, bytes::complete::tag, bytes::complete::take_till, character::complete::newline,
    combinator::opt, multi::separated_list1,
};

#[derive(Clone, PartialEq, Eq)]
pub struct Case<'a> {
    pub unparsed_regex: &'a str,
    pub negated: bool,
}

fn parse_line(input: &'_ str) -> IResult<&'_ str, Case<'_>> {
    let (input, is_negated) = opt(tag("!"))(input)?;
    let is_negated = is_negated.is_some();
    let (input, _) = tag("r ")(input)?;
    let (input, unparsed_regex) = take_till(|c| c == '\n')(input)?;
    Ok((
        input,
        Case {
            unparsed_regex,
            negated: is_negated,
        },
    ))
}

fn parse_case(input: &'_ str) -> IResult<&'_ str, Case<'_>> {
    parse_line(input)
}

pub fn parse_all_cases(input: &'_ str) -> IResult<&'_ str, Vec<Case<'_>>> {
    separated_list1(newline, parse_case)(input)
}

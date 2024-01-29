use nom::{
    bytes::complete::tag,
    bytes::complete::take_till,
    character::complete::newline,
    combinator::opt,
    error::{ErrorKind, ParseError},
    multi::separated_list1,
    IResult,
};
use regex::Regex;

#[derive(Clone, Debug)]
pub struct Case {
    pub regex: Regex,
    pub negated: bool,
}

impl PartialEq for Case {
    fn eq(&self, other: &Self) -> bool {
        self.regex.as_str() == other.regex.as_str() && self.negated == other.negated
    }
}

impl Eq for Case {}

fn parse_line(input: &str) -> IResult<&str, Case> {
    let (input, is_negated) = opt(tag("!"))(input)?;
    let is_negated = is_negated.is_some();
    let (input, _) = tag("r ")(input)?;
    let (input, content) = take_till(|c| c == '\n')(input)?;
    let regex = Regex::new(content);
    Ok((
        input,
        Case {
            regex: regex.map_err(|_| {
                println!("regex {} failed to compile", content);
                nom::Err::Failure(ParseError::from_error_kind(input, ErrorKind::Fail))
            })?,
            negated: is_negated,
        },
    ))
}

fn parse_case(input: &str) -> IResult<&str, Case> {
    parse_line(input)
}

pub fn parse_all_cases(input: &str) -> IResult<&str, Vec<Case>> {
    separated_list1(newline, parse_case)(input)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_case() {
        let test = "!r 1234";

        let result = parse_case(test).unwrap();
        assert_eq!(
            result,
            (
                "",
                Case {
                    regex: Regex::new("1234").unwrap(),
                    negated: true,
                }
            )
        );
    }

    #[test]
    fn test_parse_all_cases() {
        let test = "!r 1234\nr 4321";

        let result = parse_all_cases(test).unwrap();
        assert_eq!(
            result,
            (
                "",
                vec![
                    Case {
                        regex: Regex::new("1234").unwrap(),
                        negated: true,
                    },
                    Case {
                        regex: Regex::new("4321").unwrap(),
                        negated: false,
                    },
                ]
            )
        );
    }
}

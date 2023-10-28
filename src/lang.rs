use nom::{
    bytes::complete::tag,
    bytes::complete::take_till,
    character::complete::{char, newline},
    combinator::{opt, value},
    multi::many1,
    sequence::{terminated, tuple},
    IResult,
};

///
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Regex,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    kind: Kind,
    negated: bool,
    content: String,
}

fn parser(input: &str) -> IResult<&str, Vec<Line>> {
    many1(parse_line)(input)
}

fn parse_line(input: &str) -> IResult<&str, Line> {
    terminated(
        tuple((
            opt(tag("!")),
            terminated(value(Kind::Regex, tag("r")), char(' ')),
            take_till(|c| c == '\n'),
        )),
        newline,
    )(input)
    .map(|(remaining, (negated, kind, content))| {
        (
            remaining,
            Line {
                negated: negated.is_some(),
                kind,
                content: content.to_string(),
            },
        )
    })
}

pub fn parse(input: &str) -> Vec<Line> {
    parser(input.trim_start()).unwrap().1
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse() {
        let test = r#"
r 1234
!r 4321
"#;

        let result = parse(test);
        assert_eq!(
            result,
            vec![
                Line {
                    kind: Kind::Regex,
                    negated: false,
                    content: "1234".to_string(),
                },
                Line {
                    kind: Kind::Regex,
                    negated: true,
                    content: "4321".to_string(),
                },
            ]
        );
    }
}

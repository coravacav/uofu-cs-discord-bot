use std::io::Read;

use crate::{config::ReactRole, data::AppState};
use color_eyre::eyre::{Context, OptionExt, Result};
use poise::serenity_prelude::{self as serenity};
use serenity::Message;
pub async fn text_detection(
    ctx: &serenity::Context,
    data: &AppState,
    message: &Message,
) -> Result<()> {
    if message.is_own(ctx) {
        return Ok(());
    }

    let author_id: u64 = message.author.id.into();

    let author_has_role = data
        .config
        .read()
        .await
        .bot_react_role_members
        .iter()
        .find(|member| matches!(member, ReactRole { user_id, .. } if *user_id == author_id))
        .map(|member| member.react);

    if let Some(false) = author_has_role {
        return Ok(());
    }

    let author_has_role = message
        .author
        .has_role(
            ctx,
            message.guild_id.ok_or_eyre("should have guild id")?,
            data.config.read().await.bot_react_role_id,
        )
        .await
        .context("Couldn't get roles")?;

    data.config
        .write()
        .await
        .bot_react_role_members
        .push(ReactRole {
            user_id: author_id,
            react: author_has_role,
        });

    if let Some(message_response) = data.find_response(&message.content, &message.link()).await {
        data.run_action(&message_response, message, ctx).await?;
    }

    Ok(())
}
#[derive(Debug, Clone, Copy)]
struct Star((i64, i64));

struct Input {
    galactic_diameter: i64,
    stars: Vec<Star>,
}

type Output = Option<usize>;

fn parse_input() -> Input {
    use std::io::BufRead;
    let mut stdin = std::io::BufReader::new(std::io::stdin().lock()).bytes();

    let intermediate = stdin.next().unwrap().unwrap();

    fn parse_i64(s: &[u8]) -> i64 {
        s.iter().fold(0, |acc, &c| acc * 10 + (c - b'0') as i64)
    }

    let mut ints = intermediate.split(|&c| c == b' ').map(parse_i64);

    let galactic_diameter = ints.next().unwrap();
    let star_count = ints.next().unwrap() as usize;

    let mut stars = Vec::with_capacity(star_count);

    stdin
        .take(star_count)
        .map(Result::unwrap)
        .map(|line| {
            let mut iter = line.split(|&c| c == b' ');
            let a = parse_i64(iter.next().unwrap());
            let b = parse_i64(iter.next().unwrap());

            Star((a, b))
        })
        .for_each(|star| stars.push(star));

    Input {
        galactic_diameter,
        stars,
    }
}

fn solve(
    Input {
        galactic_diameter,
        stars,
    }: &Input,
) -> Output {
    let galactic_diameter_squared = galactic_diameter.pow(2);

    solve_recursive(galactic_diameter_squared, stars).and_then(|star| {
        let count = stars
            .iter()
            .filter(|other| distance_squared(&star.0, &other.0) <= galactic_diameter_squared)
            .count();

        (count > stars.len() / 2).then_some(count)
    })
}

fn solve_recursive(galactic_diameter_squared: i64, stars: &[Star]) -> Option<Star> {
    match stars.len() {
        0 => return None,
        1 => return Some(stars[0]),
        _ => solve_recursive(
            galactic_diameter_squared,
            &stars
                .iter()
                .rev()
                .step_by(2)
                .zip(stars.iter().rev().skip(1).step_by(2))
                .filter(|(&first, &second)| {
                    distance_squared(&first.0, &second.0) <= galactic_diameter_squared
                })
                .map(|(&first, _)| first)
                .collect::<Vec<_>>(),
        )
        .or(stars.first().copied()),
    }
}

fn distance_squared((ax, ay): &(i64, i64), (bx, by): &(i64, i64)) -> i64 {
    (ax - bx).pow(2) + (ay - by).pow(2)
}

fn main() {
    match solve(&parse_input()) {
        Some(star_count) => println!("{star_count}"),
        None => println!("NO"),
    }
}

#[test]
fn sample_input_1() {
    let input = Input {
        galactic_diameter: 10,
        stars: vec![
            Star((45, 46)),
            Star((90, 47)),
            Star((45, 54)),
            Star((90, 43)),
        ],
    };
    let output = solve(&input);

    assert!(output.is_none());
}

#[test]
fn sample_input_2() {
    let input = Input {
        galactic_diameter: 20,
        stars: vec![
            Star((1, 1)),
            Star((100, 100)),
            Star((1, 3)),
            Star((101, 101)),
            Star((3, 1)),
            Star((102, 102)),
            Star((3, 3)),
        ],
    };
    let output = solve(&input).unwrap();

    assert_eq!(output, 4);
}

#[test]
fn recursive_majority() {
    let input = Input {
        galactic_diameter: 20,
        stars: vec![
            Star((3, 3)),
            Star((1, 3)),
            Star((100, 100)),
            Star((102, 102)),
            Star((1, 1)),
            Star((3, 1)),
        ],
    };
    let output = solve(&input).unwrap();

    assert_eq!(output, 4);
}

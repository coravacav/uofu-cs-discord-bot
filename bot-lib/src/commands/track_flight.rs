use crate::data::PoiseContext;
use chrono::{Datelike, Local};
use color_eyre::eyre::{Result, eyre};
use poise::{
    CreateReply,
    serenity_prelude::{
        CreateEmbed, CreateEmbedFooter,
        colours::{branding::BLURPLE, roles::BLUE},
    },
};
use serde::Deserialize;
use std::f64::consts::PI;

#[derive(Debug, Deserialize)]
struct AirportResponse {
    response: Option<Vec<AirportData>>,
    error: Option<AirlabsError>,
}

#[derive(Debug, Deserialize)]
struct AirportData {
    lat: Option<f64>,
    lng: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct FlightResponse {
    response: Option<FlightData>,
    error: Option<AirlabsError>,
}

#[derive(Debug, Deserialize)]
struct FlightData {
    flight_iata: Option<String>,
    flight_icao: Option<String>,
    airline_iata: Option<String>,
    dep_iata: Option<String>,
    arr_iata: Option<String>,
    status: Option<String>,
    duration: Option<i64>,
    model: Option<String>,
    manufacture: Option<String>,
    dep_time: Option<String>,
    arr_time: Option<String>,
    engine: Option<String>,
    built: Option<i64>,
    speed: Option<i64>,
    alt: Option<i64>,
    arr_estimated: Option<String>,
    dep_estimated: Option<String>,
    airline_icao: Option<String>,
    dep_icao: Option<String>,
    arr_icao: Option<String>,
    airline_name: Option<String>,
    lat: Option<f64>,
    lng: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct AirlabsError {
    message: String,
}

static IATA_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[A-Z]{2}[0-9]{1,4}$").unwrap());

static ICAO_RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[A-Z]{3}[0-9]{1,4}$").unwrap());

static IATA_RE_AIRP: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[A-Z]{3}$").unwrap());

static ICAO_RE_AIRP: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"^[A-Z]{4}$").unwrap());

fn format_time(label: &str, scheduled: &Option<String>, estimated: &Option<String>) -> String {
    match (scheduled, estimated) {
        (_, Some(est)) => format!("Est {label}: {est}"),
        (Some(sched), None) => format!("{label}: {sched}"),
        _ => format!("{label}: N/A"),
    }
}

fn minutes_to_hours(duration: Option<i64>) -> String {
    let time = duration.unwrap_or(0);
    let hours = time / 60;
    let minutes = time % 60;

    format!("{hours}h {minutes}m")
}

fn progress_bar(percentage: f64) -> String {
    let total_blocks = 10;
    let clamped_percentage = percentage.clamp(0.0, 1.0);
    let filled_blocks = (clamped_percentage * total_blocks as f64).round() as usize;
    let empty_blocks = total_blocks - filled_blocks;

    let filled_part = "=".repeat(filled_blocks);
    let empty_part = "-".repeat(empty_blocks);
    let leading_char = if filled_blocks < total_blocks {
        "✈️"
    } else {
        ""
    };

    format!("{filled_part}{leading_char}{empty_part}")
}

fn degree_to_rad(deg: f64) -> f64 {
    deg * PI / 180.0
}

fn haversine_distance(lat1: f64, long1: f64, lat2: f64, long2: f64) -> f64 {
    let r = 6371.0;
    let dlat = degree_to_rad(lat2 - lat1);
    let dlon = degree_to_rad(long2 - long1);
    let lat1 = degree_to_rad(lat1);
    let lat2 = degree_to_rad(lat2);

    let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

fn flight_progess(
    plane_lat: f64,
    plane_long: f64,
    source_lat: f64,
    source_long: f64,
    dst_lat: f64,
    dst_long: f64,
) -> f64 {
    let total_distance = haversine_distance(source_lat, source_long, dst_lat, dst_long);
    let traveled_distance = haversine_distance(source_lat, source_long, plane_lat, plane_long);
    if total_distance == 0.0 {
        return 0.0;
    }
    (traveled_distance / total_distance).clamp(0.0, 1.0)
}

async fn airport_lookup(api_key: &str, code: &str) -> Result<AirportData> {
    let searched_iata = IATA_RE_AIRP.is_match(code);
    let searched_icao = ICAO_RE_AIRP.is_match(code);

    let url = if searched_iata {
        format!("https://airlabs.co/api/v9/airports?iata_code={code}&api_key={api_key}")
    } else if searched_icao {
        format!("https://airlabs.co/api/v9/airports?icao_code={code}&api_key={api_key}")
    } else {
        return Err(eyre!("Invalid airport code: {code}"));
    };

    let client = reqwest::Client::new();
    let response: AirportResponse = client.get(url).send().await?.json().await?;

    if let Some(err) = response.error {
        return Err(eyre!("API Error: {}", err.message));
    }

    let airports = response
        .response
        .ok_or_else(|| eyre!("No airport data found for code: {code}"))?;

    airports
        .into_iter()
        .next()
        .ok_or_else(|| eyre!("Airport list was empty for code: {code}"))
}

async fn flight_lookup(ctx: PoiseContext<'_>, api_key: &str, code: &str) -> Option<FlightData> {
    let date = Local::now().format("%Y-%m-%d").to_string();

    let searched_iata = IATA_RE.is_match(code);
    let searched_icao = ICAO_RE.is_match(code);

    let url = if searched_iata {
        format!(
            "https://airlabs.co/api/v9/flight?flight_iata={code}&api_key={api_key}&flight_date={date}"
        )
    } else if searched_icao {
        format!(
            "https://airlabs.co/api/v9/flight?flight_icao={code}&api_key={api_key}&flight_date={date}"
        )
    } else {
        ctx.reply("Please provide a valid flight number (IATA or ICAO)")
            .await
            .ok()?;
        return None;
    };

    let client = reqwest::Client::new();
    let response: FlightResponse = client.get(url).send().await.ok()?.json().await.ok()?;

    if let Some(err) = response.error {
        ctx.reply(format!("API Error: {}", err.message))
            .await
            .ok()?;
        return None;
    }

    let Some(flight) = response.response else {
        ctx.reply("No flight data found for that number.")
            .await
            .ok()?;
        return None;
    };

    Some(flight)
}

///get information on a specified flight
#[poise::command(slash_command, rename = "trackflight")]
pub async fn track_flight(ctx: PoiseContext<'_>, search: String) -> Result<()> {
    let Ok(api_key) = std::env::var("AIRLABS_API_KEY") else {
        ctx.reply("Cannot track flight, missing API key.").await?;
        return Ok(());
    };

    ctx.defer().await?;

    let search: String = search
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_uppercase();

    let searched_iata = IATA_RE.is_match(&search);
    let searched_icao = ICAO_RE.is_match(&search);

    let flight = match flight_lookup(ctx, &api_key, &search).await {
        Some(flight) => flight,
        None => return Ok(()),
    };

    let (flight_label, dep_airport, arr_airport) = if searched_iata {
        (
            flight
                .flight_iata
                .or(flight.flight_icao)
                .unwrap_or_default(),
            flight.dep_iata.or(flight.dep_icao),
            flight.arr_iata.or(flight.arr_icao),
        )
    } else if searched_icao {
        (
            flight
                .flight_icao
                .or(flight.flight_iata)
                .unwrap_or_default(),
            flight.dep_icao.or(flight.dep_iata),
            flight.arr_icao.or(flight.arr_iata),
        )
    } else {
        (
            flight
                .flight_iata
                .or(flight.flight_icao)
                .unwrap_or_default(),
            flight.dep_iata.or(flight.dep_icao),
            flight.arr_iata.or(flight.arr_icao),
        )
    };

    let Some(dep_code) = dep_airport else {
        ctx.reply("Departure airport code not available for this flight.")
            .await?;
        return Ok(());
    };
    let Some(arr_code) = arr_airport else {
        ctx.reply("Arrival airport code not available for this flight.")
            .await?;
        return Ok(());
    };

    let speed = (flight.speed.unwrap_or(0) as f64 * 0.5399568) as i64;
    let altitude = (flight.alt.unwrap_or(0) as f64 * 3.28084) as i64;
    let timestamp = chrono::Utc::now();
    let dep_time_display = format_time("Departure Time", &flight.dep_time, &flight.dep_estimated);
    let arr_time_display = format_time("Arrival Time", &flight.arr_time, &flight.arr_estimated);
    let airline = flight.airline_name.unwrap_or_else(|| "Unknown".to_string());
    let status = flight.status.unwrap_or_else(|| "Unknown".to_string());
    let duration = minutes_to_hours(flight.duration);

    let mut embed = CreateEmbed::new()
        .title(format!("Flight {flight_label}"))
        .url(format!(
            "https://www.flightradar24.com/data/flights/{flight_label}"
        ))
        .field("Airline", airline, true)
        .field("\u{200B}", "\u{200B}", true)
        .field("Status", &status, true)
        .field("Route", format!("{dep_code} -> {arr_code}"), true)
        .field("\u{200B}", "\u{200B}", true)
        .field("Duration", duration, true)
        .field("🛫 Departure", dep_time_display, false)
        .field("🛬 Arrival", arr_time_display, false);

    if status == "en-route" {
        let depart_airport = match airport_lookup(&api_key, &dep_code).await {
            Ok(airport) => airport,
            Err(e) => {
                ctx.reply(format!(
                    "Failed to lookup departure airport {dep_code}: {e}"
                ))
                .await?;
                return Ok(());
            }
        };

        let arrival_airport = match airport_lookup(&api_key, &arr_code).await {
            Ok(airport) => airport,
            Err(e) => {
                ctx.reply(format!("Failed to lookup arrival airport {arr_code}: {e}"))
                    .await?;
                return Ok(());
            }
        };

        let aicraft_lat = flight.lat.unwrap_or(0.0);
        let aicraft_long = flight.lng.unwrap_or(0.0);
        let source_airport_lat = depart_airport.lat.unwrap_or(0.0);
        let source_airport_long = depart_airport.lng.unwrap_or(0.0);
        let arrival_airport_lat = arrival_airport.lat.unwrap_or(0.0);
        let arrival_airport_long = arrival_airport.lng.unwrap_or(0.0);
        let progress = flight_progess(
            aicraft_lat,
            aicraft_long,
            source_airport_lat,
            source_airport_long,
            arrival_airport_lat,
            arrival_airport_long,
        );

        let progress_bar = progress_bar(progress);

        embed = embed
            .field("Speed", speed.to_string() + "kts", true)
            .field("\u{200B}", "\u{200B}", true)
            .field("Altitude", altitude.to_string() + "ft", true)
            .field(
                "Progress",
                format!("{} {} {}", &dep_code, progress_bar, &arr_code),
                false,
            )
            .timestamp(timestamp)
            .footer(CreateEmbedFooter::new("Data provided by AirLabs API"))
            .color(BLUE);
    } else {
        embed = embed
            .timestamp(timestamp)
            .footer(CreateEmbedFooter::new("Data provided by AirLabs API"))
            .color(BLUE);
    }

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

///get information on an aircraft
#[poise::command(slash_command, rename = "planeinfo")]
pub async fn plane_details(ctx: PoiseContext<'_>, search: String) -> Result<()> {
    let Ok(api_key) = std::env::var("AIRLABS_API_KEY") else {
        ctx.reply("Cannot get plane details, missing API key.")
            .await?;
        return Ok(());
    };

    ctx.defer().await?;

    let search: String = search
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_uppercase();

    let searched_iata = IATA_RE.is_match(&search);
    let searched_icao = ICAO_RE.is_match(&search);

    let flight = match flight_lookup(ctx, &api_key, &search).await {
        Some(flight) => flight,
        None => return Ok(()),
    };

    let (flight_label, airline) = if searched_iata {
        (
            flight
                .flight_iata
                .or(flight.flight_icao)
                .unwrap_or_default(),
            flight.airline_iata.unwrap_or_else(|| "N/A".to_string()),
        )
    } else if searched_icao {
        (
            flight
                .flight_icao
                .or(flight.flight_iata)
                .unwrap_or_default(),
            flight.airline_icao.unwrap_or_else(|| "N/A".to_string()),
        )
    } else {
        (
            flight
                .flight_iata
                .or(flight.flight_icao)
                .unwrap_or_default(),
            flight.airline_iata.unwrap_or_else(|| "N/A".to_string()),
        )
    };

    let status = flight.status.as_deref().unwrap_or("Unknown");
    let aircraft = flight.model.as_deref().unwrap_or("BoingBus 67420 Max");
    let manufacture = flight.manufacture.as_deref().unwrap_or("BoingBus");
    let engine = flight.engine.as_deref().unwrap_or("FartJet");
    let built = flight.built.unwrap_or(0);
    let current_date = chrono::Utc::now();
    let age = current_date.year() - built as i32;

    let embed = CreateEmbed::new()
        .title(format!("Aircraft Details for Flight {flight_label}"))
        .url(format!(
            "https://www.flightradar24.com/data/flights/{flight_label}"
        ))
        .field("Airline", airline, true)
        .field("Status", status, true)
        .field("\u{200B}", "\u{200B}", true)
        .field("Aircraft Type", aircraft, true)
        .field("Manufacture", manufacture, true)
        .field("Engine Type", engine, true)
        .field("Age", format!("{age} years"), true)
        .field("Date of Manufacture", built.to_string(), true)
        .timestamp(chrono::Utc::now())
        .footer(CreateEmbedFooter::new("Data provided by AirLabs API"))
        .color(BLURPLE);

    ctx.send(CreateReply::default().embed(embed)).await?;

    Ok(())
}

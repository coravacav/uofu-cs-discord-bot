use std::f64::consts::PI;
use crate::data::PoiseContext;
use color_eyre::eyre::{Result, eyre};
use poise::{CreateReply};
use regex::Regex;
use serde::Deserialize;
use chrono::{Local, Datelike};

#[derive(Debug, Deserialize)]
struct AirportResponse {
    response: Option<AirportData>,
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
    age: Option<i64>,
    built: Option<i64>,
    arr_estimated: Option<String>,
    dep_estimated: Option<String>,
    flight_icao: Option<String>,
    airline_icao: Option<String>,
    dep_icao: Option<String>,
    arr_icao: Option<String>,
    airline_name: Option<String>,
    lat: Option<f64>,
    long: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct AirlabsError {
    message: String,
}

fn format_time(label: &str, scheduled: &Option<String>, estimated: &Option<String>) -> String {
    match (scheduled, estimated) {
        (_, Some(est)) => format!("Est {label}: {est}"),
        (Some(sched), None) => format!("{label}: {sched}"),
        _ => format!("{label}: N/A"),
    }
}

fn minutes_to_hours(duration: Option<i64>) -> String {
    let time = duration.unwrap_or(0);
    let hours = time/60;
    let minutes = time%60;

    format!("Flight Time: {hours}:{minutes}")
}

fn degree_to_rad(deg :f64) -> f64{
    deg * PI / 180.0
}

fn haversine_distance(lat1: f64, long1: f64, lat2: f64, long2: f64) -> f64 {
    let r = 6371.0;
    let dlat = degree_to_rad(lat2 - lat1);
    let dlon = degree_to_rad(long2 - long1);
    let lat1 = degree_to_rad(lat1);
    let lat2 = degree_to_rad(lat2);

    let a = (dlat/2.0).sin().powi(2) +
            lat1.cos() * lat2.cos() * (dlon/2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

fn flight_progess(plane_lat: f64, plane_long: f64, source_lat: f64, source_long: f64, dst_lat: f64, dst_long: f64) -> f64{
    let d_star_dest = haversine_distance(source_lat, source_long, dst_lat, dst_long);
    let d_start_airp = haversine_distance(source_lat, source_long, plane_lat, plane_long);
    d_star_dest / d_start_airp
}

// async fn airport_lookup(ctx: PoiseContext<'_>, code: String) -> Result<()> {
//     let api_key = std::env::var("API_KEY").map_err(|_| eyre!("API_KEY missing from environment"))?;
//     let iata_ap = Regex::new(r"^[A-Z]{2}").unwrap();
//     let icao_ap = Regex::new(r"^[A-Z]{3}").unwrap();
//     let searched_iata = iata_ap.is_match(&code);
//     let searched_icao = icao_ap.is_match(&code);

//     let url = if searched_iata {
//         format!(
//             "https://airlabs.co/api/v9/airport?iata_code={}&api_key={}",
//             code, api_key
//         )
//     } else if searched_icao {
//         format!(
//             "https://airlabs.co/api/v9/airport?icao_code={}&api_key={}",
//             code, api_key
//         )
//     } else {
//         ctx.reply("Please provide a valid airport ident(IATA or ICAO)").await?;
//         return Ok(());
//     };

//     let client = reqwest::Client::new();
//     let response: FlightResponse = client
//         .get(url)
//         .send()
//         .await?
//         .json()
//         .await?;

//     if let Some(err) = response.error {
//         ctx.reply(format!("API Error: {}", err.message)).await?;
//         return Ok(());
//     }

//     let Some(flight) = response.response else {
//         ctx.reply("No airport data found for that code.").await?;
//         return Ok(());
//     };

// }

///get information on a specified flight
#[poise::command(slash_command, rename = "trackflight")]
pub async fn track_flight(
    ctx: PoiseContext<'_>,

    search: String,
) -> Result<()> {
    ctx.defer().await?;

    let search: String = search
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_uppercase();

    let api_key = std::env::var("API_KEY").map_err(|_| eyre!("API_KEY missing from environment"))?;

    let iata_re = Regex::new(r"^[A-Z]{2}[0-9]{1,4}").unwrap();
    let icao_re = Regex::new(r"^[A-Z]{3}[0-9]{1,4}").unwrap();
    let date = Local::now().format("%Y-%m-%d").to_string();

    let searched_iata = iata_re.is_match(&search);
    let searched_icao = icao_re.is_match(&search);

    let url = if searched_iata {
        format!(
            "https://airlabs.co/api/v9/flight?flight_iata={}&api_key={}&flight_date={}",
            search, api_key, date
        )
    } else if searched_icao {
        format!(
            "https://airlabs.co/api/v9/flight?flight_icao={}&api_key={}&flight_date={}",
            search, api_key, date
        )
    } else {
        ctx.reply("Please provide a valid flight number (IATA or ICAO)").await?;
        return Ok(());
    };

    let client = reqwest::Client::new();
    let response: FlightResponse = client
        .get(url)
        .send()
        .await?
        .json()
        .await?;

    if let Some(err) = response.error {
        ctx.reply(format!("API Error: {}", err.message)).await?;
        return Ok(());
    }

    let Some(flight) = response.response else {
        ctx.reply("No flight data found for that number.").await?;
        return Ok(());
    };

    let (flight_label, airline_standard, dep_airport, arr_airport) = if searched_iata {
        (
            flight.flight_iata.clone().or(flight.flight_icao.clone()).unwrap_or_default(),
            flight.airline_iata.clone().or(flight.airline_iata.clone()).unwrap_or_else(|| "N/A".to_string()),
            flight.dep_iata.clone().or(flight.dep_icao.clone()).unwrap_or_else(|| "N/A".to_string()),
            flight.arr_iata.clone().or(flight.arr_icao.clone()).unwrap_or_else(|| "N/A".to_string()),
        )
    } else if searched_icao {
        (
            flight.flight_icao.clone().or(flight.flight_iata.clone()).unwrap_or_default(),
            flight.airline_icao.clone().or(flight.airline_icao.clone()).unwrap_or_else(|| "N/A".to_string()),
            flight.dep_icao.clone().or(flight.dep_iata.clone()).unwrap_or_else(|| "N/A".to_string()),
            flight.arr_icao.clone().or(flight.arr_iata.clone()).unwrap_or_else(|| "N/A".to_string()),
        )
    } else {
        (
            flight.flight_iata.clone().or(flight.flight_icao.clone()).unwrap_or_default(),
            flight.airline_iata.clone().or(flight.airline_iata.clone()).unwrap_or_else(|| "N/A".to_string()),
            flight.dep_iata.clone().or(flight.dep_icao.clone()).unwrap_or_else(|| "N/A".to_string()),
            flight.arr_iata.clone().or(flight.arr_icao.clone()).unwrap_or_else(|| "N/A".to_string()),
        )
    };

    let dep_time_display = format_time("Departure Time", &flight.dep_time, &flight.dep_estimated);
    let arr_time_display = format_time("Arrival Time", &flight.arr_time, &flight.arr_estimated);
    let airline = flight.airline_name.clone().unwrap_or_else(|| "Unknown".to_string());
    let status = flight.status.clone().unwrap_or_else(|| "Unknown".to_string());
    let duration = minutes_to_hours(flight.duration.clone());

    let reply_msg = format!(
        "**Flight {}\n**Airline: {}\nStatus: {}\n{} -> {}\n{}\n{}\n{}",
        flight_label, airline, status, dep_airport, arr_airport, duration, dep_time_display, arr_time_display
    );

    ctx.send(CreateReply::default().content(reply_msg)).await?;

    Ok(())
}

///get information on an aircraft
#[poise::command(slash_command, rename = "planeinfo")]
pub async fn plane_details(
    ctx: PoiseContext<'_>,

    search: String,
) -> Result<()> {
    ctx.defer().await?;

    let search: String = search
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_uppercase();

    let api_key = std::env::var("API_KEY").map_err(|_| eyre!("API_KEY missing from environment"))?;

    let iata_re = Regex::new(r"^[A-Z]{2}[0-9]{1,4}").unwrap();
    let icao_re = Regex::new(r"^[A-Z]{3}[0-9]{1,4}").unwrap();
    let date = Local::now().format("%Y-%m-%d").to_string();

    let searched_iata = iata_re.is_match(&search);
    let searched_icao = icao_re.is_match(&search);

    let url = if searched_iata {
        format!(
            "https://airlabs.co/api/v9/flight?flight_iata={}&api_key={}&flight_date={}",
            search, api_key, date
        )
    } else if searched_icao {
        format!(
            "https://airlabs.co/api/v9/flight?flight_icao={}&api_key={}&flight_date={}",
            search, api_key, date
        )
    } else {
        ctx.reply("Please provide a valid flight number (IATA or ICAO)").await?;
        return Ok(());
    };

    let client = reqwest::Client::new();
    let response: FlightResponse = client
        .get(url)
        .send()
        .await?
        .json()
        .await?;

    if let Some(err) = response.error {
        ctx.reply(format!("API Error: {}", err.message)).await?;
        return Ok(());
    }

    let Some(flight) = response.response else {
        ctx.reply("No flight data found for that number.").await?;
        return Ok(());
    };

    let (flight_label, airline) = if searched_iata {
        (
            flight.flight_iata.clone().or(flight.flight_icao.clone()).unwrap_or_default(),
            flight.airline_iata.clone().or(flight.airline_iata.clone()).unwrap_or_else(|| "N/A".to_string()),
        )
    } else if searched_icao {
        (
            flight.flight_icao.clone().or(flight.flight_iata.clone()).unwrap_or_default(),
            flight.airline_icao.clone().or(flight.airline_icao.clone()).unwrap_or_else(|| "N/A".to_string()),
        )
    } else {
        (
            flight.flight_iata.clone().or(flight.flight_icao.clone()).unwrap_or_default(),
            flight.airline_iata.clone().or(flight.airline_iata.clone()).unwrap_or_else(|| "N/A".to_string()),
        )
    };

    let status = flight.status.clone().unwrap_or_else(|| "Unknown".to_string());
    let aircraft = flight.model.clone().unwrap_or_else(|| "BoingBus 67420 Max".to_string());
    let manufacture = flight.manufacture.clone().unwrap_or_else(|| "BoingBus".to_string());
    let engine = flight.engine.clone().unwrap_or_else(|| "FartJet".to_string());
    let built = flight.built.clone().unwrap_or_else(|| 0);
    let current_date = chrono::Utc::now();
    let age = current_date.year() - built as i32;

    let reply_msg = format!(
        "**Flight {}\n**Airline: {}\nStatus: {}\nAircraft Type: {}\nManufacture: {}\nEngine Type: {}\nAge: {}\nDate of Manufacture: {}",
        flight_label, airline, status, aircraft, manufacture, engine, age, built
    );

    ctx.send(CreateReply::default().content(reply_msg)).await?;

    Ok(())
}
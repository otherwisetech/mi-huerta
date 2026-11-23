//! Riego inteligente's weather: Open-Meteo (free, no key, no account), **only when the user
//! turns it on**. What is sent, and to whom, is exactly [`forecast_url`] / [`geocode_url`]:
//! a coarse location (latitude and longitude rounded to one decimal, about 11 km) or the city
//! the user typed, to `api.open-meteo.com` / `geocoding-api.open-meteo.com`. No plant, no name,
//! no identifier. Without it, the user logs rain by hand.
use crate::model::{Forecast, ForecastDay, Garden, RainSource};
use chrono::NaiveDate;
use serde_json::Value;
use std::time::Duration;

pub const FORECAST_HOST: &str = "api.open-meteo.com";
pub const GEOCODE_HOST: &str = "geocoding-api.open-meteo.com";
pub const COORD_DECIMALS: i32 = 1;

pub fn round_coord(x: f64) -> f64 {
    let f = 10f64.powi(COORD_DECIMALS);
    (x * f).round() / f
}

fn encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Yesterday's measured rain plus today and the next two days, at the rounded location.
pub fn forecast_url(lat: f64, lon: f64) -> String {
    format!("https://{FORECAST_HOST}/v1/forecast?latitude={:.1}&longitude={:.1}&daily=precipitation_sum,precipitation_probability_max&past_days=1&forecast_days=3&timezone=auto", round_coord(lat), round_coord(lon))
}

/// A city search (the user's own words), in the app's language.
pub fn geocode_url(name: &str, lang: &str) -> String {
    format!("https://{GEOCODE_HOST}/v1/search?name={}&count=5&language={}&format=json", encode(name.trim()), encode(lang))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub name: String,
    /// Department / state and country, for telling places apart.
    pub region: String,
    pub lat: f64,
    pub lon: f64,
}

pub fn parse_geocode(body: &str) -> Result<Vec<Found>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    Ok(v.get("results")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|r| {
                    let region = [r.get("admin1"), r.get("country")].iter().filter_map(|x| x.and_then(Value::as_str)).collect::<Vec<_>>().join(", ");
                    Some(Found { name: r.get("name")?.as_str()?.to_string(), region, lat: round_coord(r.get("latitude")?.as_f64()?), lon: round_coord(r.get("longitude")?.as_f64()?) })
                })
                .collect()
        })
        .unwrap_or_default())
}

pub fn parse_forecast(body: &str, fetched_at: &str) -> Result<Forecast, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    if let Some(r) = v.get("reason").and_then(Value::as_str) {
        return Err(r.to_string());
    }
    let daily = v.get("daily").ok_or("no daily data")?;
    let times = daily.get("time").and_then(Value::as_array).ok_or("no days")?;
    let rain = daily.get("precipitation_sum").and_then(Value::as_array);
    let prob = daily.get("precipitation_probability_max").and_then(Value::as_array);
    let mut days = Vec::new();
    for (i, t) in times.iter().enumerate() {
        let Some(date) = t.as_str().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()) else { continue };
        let rain_mm = rain.and_then(|r| r.get(i)).and_then(Value::as_f64).unwrap_or(0.0);
        days.push(ForecastDay { date, rain_mm, probability: prob.and_then(|p| p.get(i)).and_then(Value::as_f64) });
    }
    Ok(Forecast { fetched_at: fetched_at.to_string(), days })
}

/// Keeps the forecast and logs the days already past (measured rain) as Open-Meteo rain.
pub fn apply(g: &mut Garden, f: Forecast, today: NaiveDate) {
    for d in f.days.iter().filter(|d| d.date < today) {
        g.log_rain(d.date, Some(d.rain_mm), RainSource::OpenMeteo);
    }
    g.forecast = Some(f);
}

fn get(url: &str) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).build().into();
    let mut resp = agent.get(url).header("User-Agent", concat!("mi-huerta/", env!("CARGO_PKG_VERSION"))).call().map_err(|e| e.to_string())?;
    resp.body_mut().read_to_string().map_err(|e| e.to_string())
}

/// Network: only call when the user turned weather on.
pub fn fetch_forecast(lat: f64, lon: f64) -> Result<Forecast, String> {
    parse_forecast(&get(&forecast_url(lat, lon))?, &chrono::Utc::now().to_rfc3339())
}

/// Network: only on the user's explicit search.
pub fn search(name: &str, lang: &str) -> Result<Vec<Found>, String> {
    parse_geocode(&get(&geocode_url(name, lang))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_rounded_location_is_sent() {
        let u = forecast_url(6.244_203, -75.581_212);
        assert!(u.starts_with("https://api.open-meteo.com/v1/forecast?"));
        assert!(u.contains("latitude=6.2&longitude=-75.6"), "{u}");
        assert!(!u.contains("6.24"));
        assert_eq!(geocode_url(" Santa Fe de Antioquia ", "es"), "https://geocoding-api.open-meteo.com/v1/search?name=Santa%20Fe%20de%20Antioquia&count=5&language=es&format=json");
        assert_eq!(round_coord(4.6097), 4.6);
    }

    #[test]
    fn parses_and_applies_a_forecast() {
        let body = r#"{"latitude":4.6,"daily":{"time":["2026-11-22","2026-11-23","2026-11-24","2026-11-25"],"precipitation_sum":[7.3,0.0,12.4,null],"precipitation_probability_max":[90,10,85,null]}}"#;
        let f = parse_forecast(body, "2026-11-23T05:00:00Z").unwrap();
        assert_eq!(f.days.len(), 4);
        assert_eq!(f.days[2].rain_mm, 12.4);
        assert_eq!(f.days[3].probability, None);
        let mut g = Garden::default();
        let today = NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        apply(&mut g, f, today);
        assert_eq!(g.rain.len(), 1, "only the past day is logged as rain");
        assert_eq!((g.rain[0].mm, g.rain[0].source), (Some(7.3), RainSource::OpenMeteo));
        assert!(parse_forecast(r#"{"error":true,"reason":"Latitude must be in range"}"#, "").is_err());
        let found = parse_geocode(r#"{"results":[{"name":"Medellín","latitude":6.25184,"longitude":-75.56359,"country":"Colombia","admin1":"Antioquia"}]}"#).unwrap();
        assert_eq!(found, vec![Found { name: "Medellín".into(), region: "Antioquia, Colombia".into(), lat: 6.3, lon: -75.6 }]);
        assert!(parse_geocode("{}").unwrap().is_empty());
    }
}

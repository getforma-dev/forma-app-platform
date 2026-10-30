//! Current weather for the weather widget (APP-103): Open-Meteo, no key.
//!
//! Ported from our own `forma-generative-ui` (`src/connectors/catalog/
//! environment.rs`: `geocode`, `weather`, `weather_condition`), with the
//! bounded-fetch pattern of its `src/connectors/media.rs`: a 2 s connect and
//! an 8 s overall timeout, no redirects, and a response body capped at
//! `MAX_BODY_BYTES` (checked against `Content-Length` and while streaming).
//!
//! Fixture mode (APP-098 DS-65): inside a demo organization the route always
//! answers canned, deterministic conditions, and anywhere it answers them
//! when Open-Meteo cannot be reached; a fixture answer says so
//! (`source: "fixture"`), and the card labels it.
use super::state;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

const GEOCODING_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
const WEATHER_URL: &str = "https://api.open-meteo.com/v1/forecast";
/// Both Open-Meteo answers are a few KiB; anything beyond this is refused.
pub const MAX_BODY_BYTES: usize = 64 * 1024;
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(8);

/// Why no live answer was produced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FetchError {
    /// Open-Meteo knows no such place (422 `location_not_found`).
    NotFound,
    /// Transport, status, size or shape: the route serves the fixture.
    Unavailable,
}

#[derive(Debug, Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeocodingResult>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GeocodingResult {
    name: String,
    latitude: f64,
    longitude: f64,
    timezone: String,
    country: Option<String>,
    admin1: Option<String>,
}
impl GeocodingResult {
    fn display_name(&self) -> String {
        // A region that repeats the name (Tokyo, Tokyo) gives way to the country.
        let distinct = |region: &Option<String>| region.clone().filter(|r| *r != self.name);
        match distinct(&self.admin1).or_else(|| distinct(&self.country)) {
            Some(region) => format!("{}, {}", self.name, region),
            None => self.name.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct WeatherResponse {
    current: WeatherCurrent,
}

#[derive(Debug, Deserialize)]
struct WeatherCurrent {
    temperature_2m: f64,
    apparent_temperature: f64,
    relative_humidity_2m: f64,
    precipitation: f64,
    weather_code: u16,
    wind_speed_10m: f64,
}

/// The condition text and the card's icon kind for a WMO weather code.
pub fn weather_condition(code: u16) -> (&'static str, &'static str) {
    match code {
        0 => ("Clear sky", "clear"),
        1 | 2 => ("Partly cloudy", "cloudy"),
        3 => ("Overcast", "overcast"),
        45 | 48 => ("Foggy", "fog"),
        51..=67 | 80..=82 => ("Rain", "rain"),
        71..=77 | 85 | 86 => ("Snow", "snow"),
        95..=99 => ("Thunderstorms", "storm"),
        _ => ("Mixed conditions", "cloudy"),
    }
}

/// Formats an Open-Meteo answer as the card's document.
pub(super) fn document(
    place: &str,
    current: &WeatherResponse,
    metric: bool,
    source: &str,
) -> Value {
    let current = &current.current;
    let (condition, icon) = weather_condition(current.weather_code);
    let wind_suffix = if metric { "km/h" } else { "mph" };
    let precipitation_suffix = if metric { "mm" } else { "in" };
    json!({
        "location": place,
        "temperature": format!("{:.0}°", current.temperature_2m),
        "apparent": format!("Feels like {:.0}°", current.apparent_temperature),
        "condition": condition,
        "icon": icon,
        "wind": format!("{:.0} {wind_suffix}", current.wind_speed_10m),
        "humidity": format!("{:.0}%", current.relative_humidity_2m),
        "precipitation": format!("{:.2} {precipitation_suffix}", current.precipitation),
        "units": if metric { "metric" } else { "imperial" },
        "provider_label": if source == "live" { "Open-Meteo" } else { "Sample conditions" },
        "source": source,
    })
}

/// Canned conditions (DS-65): a few rehearsed places, a mild default for
/// any other; deterministic, in the requested units.
pub fn fixture(location: &str, metric: bool) -> Value {
    // (match, display name, °F, feels °F, humidity %, mph, precipitation in, WMO code)
    type Place = (&'static str, &'static str, f64, f64, f64, f64, f64, u16);
    const PLACES: &[Place] = &[
        ("austin", "Austin, Texas", 84.0, 87.0, 48.0, 9.0, 0.0, 1),
        ("tokyo", "Tokyo, Japan", 63.0, 62.0, 66.0, 7.0, 0.02, 3),
        (
            "new york",
            "New York, New York",
            58.0,
            55.0,
            61.0,
            12.0,
            0.0,
            2,
        ),
        (
            "san francisco",
            "San Francisco, California",
            61.0,
            60.0,
            78.0,
            14.0,
            0.0,
            45,
        ),
        (
            "london",
            "London, England",
            52.0,
            49.0,
            82.0,
            11.0,
            0.08,
            61,
        ),
        ("quito", "Quito, Ecuador", 64.0, 64.0, 70.0, 6.0, 0.04, 80),
        (
            "seattle",
            "Seattle, Washington",
            54.0,
            52.0,
            85.0,
            8.0,
            0.12,
            63,
        ),
    ];
    let lower = location.to_lowercase();
    let (name, f, feels, humidity, mph, inches, code) = PLACES
        .iter()
        .find(|place| lower == place.0 || lower.starts_with(&format!("{},", place.0)))
        .map(|p| (p.1.to_owned(), p.2, p.3, p.4, p.5, p.6, p.7))
        .unwrap_or_else(|| (location.to_owned(), 72.0, 72.0, 52.0, 8.0, 0.0, 0));
    let (temperature, apparent, wind, precipitation) = if metric {
        (
            (f - 32.0) * 5.0 / 9.0,
            (feels - 32.0) * 5.0 / 9.0,
            mph * 1.609_344,
            inches * 25.4,
        )
    } else {
        (f, feels, mph, inches)
    };
    let response = WeatherResponse {
        current: WeatherCurrent {
            temperature_2m: temperature,
            apparent_temperature: apparent,
            relative_humidity_2m: humidity,
            precipitation,
            weather_code: code,
            wind_speed_10m: wind,
        },
    };
    document(&name, &response, metric, "fixture")
}

/// The Open-Meteo client: rustls, no redirects, bounded time and size.
pub struct OpenMeteo {
    http: Option<reqwest::Client>,
}
impl Default for OpenMeteo {
    fn default() -> Self {
        Self::new()
    }
}
impl OpenMeteo {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .use_rustls_tls()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(FETCH_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .ok();
        Self { http }
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<T, FetchError> {
        let http = self.http.as_ref().ok_or(FetchError::Unavailable)?;
        let mut response = http
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .query(query)
            .send()
            .await
            .map_err(|_| FetchError::Unavailable)?;
        if !response.status().is_success() {
            return Err(FetchError::Unavailable);
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_BODY_BYTES as u64)
        {
            return Err(FetchError::Unavailable);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| FetchError::Unavailable)?
        {
            if body.len() + chunk.len() > MAX_BODY_BYTES {
                return Err(FetchError::Unavailable);
            }
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&body).map_err(|_| FetchError::Unavailable)
    }

    /// Geocodes `location` and reads its current conditions, all within
    /// `FETCH_TIMEOUT`.
    pub async fn current(&self, location: &str, metric: bool) -> Result<Value, FetchError> {
        tokio::time::timeout(FETCH_TIMEOUT, self.current_unbounded(location, metric))
            .await
            .unwrap_or(Err(FetchError::Unavailable))
    }

    async fn current_unbounded(&self, location: &str, metric: bool) -> Result<Value, FetchError> {
        let geocoding: GeocodingResponse = self
            .get(
                GEOCODING_URL,
                &[
                    ("name", location),
                    ("count", "1"),
                    ("language", "en"),
                    ("format", "json"),
                ],
            )
            .await?;
        let place = geocoding
            .results
            .and_then(|results| results.into_iter().next())
            .ok_or(FetchError::NotFound)?;
        if !(place.latitude.is_finite() && place.longitude.is_finite())
            || state::location(&place.name).is_none()
            || place.timezone.len() > 64
        {
            return Err(FetchError::Unavailable);
        }
        let latitude = place.latitude.to_string();
        let longitude = place.longitude.to_string();
        let (temperature_unit, wind_speed_unit, precipitation_unit) = if metric {
            ("celsius", "kmh", "mm")
        } else {
            ("fahrenheit", "mph", "inch")
        };
        let response: WeatherResponse = self
            .get(
                WEATHER_URL,
                &[
                    ("latitude", latitude.as_str()),
                    ("longitude", longitude.as_str()),
                    (
                        "current",
                        "temperature_2m,apparent_temperature,relative_humidity_2m,precipitation,weather_code,wind_speed_10m",
                    ),
                    ("temperature_unit", temperature_unit),
                    ("wind_speed_unit", wind_speed_unit),
                    ("precipitation_unit", precipitation_unit),
                    ("timezone", place.timezone.as_str()),
                ],
            )
            .await?;
        let current = &response.current;
        if ![
            current.temperature_2m,
            current.apparent_temperature,
            current.relative_humidity_2m,
            current.precipitation,
            current.wind_speed_10m,
        ]
        .iter()
        .all(|v| v.is_finite() && v.abs() < 10_000.0)
        {
            return Err(FetchError::Unavailable);
        }
        let name = place.display_name();
        let name = state::location(&name).unwrap_or_else(|| place.name.clone());
        Ok(document(&name, &response, metric, "live"))
    }
}

#[cfg(test)]
pub(super) fn parse_answers(geocoding: &str, weather: &str) -> Option<(String, WeatherResponse)> {
    let geocoding: GeocodingResponse = serde_json::from_str(geocoding).ok()?;
    let place = geocoding.results?.into_iter().next()?;
    let weather: WeatherResponse = serde_json::from_str(weather).ok()?;
    Some((place.display_name(), weather))
}

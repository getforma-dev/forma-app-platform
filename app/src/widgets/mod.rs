//! APP-103 (ADR-011 pitch): canvas utility widgets, a weather card, a
//! calculator and a clock, that a member places with `/weather`,
//! `/calculator` and `/clock` and restyles in words.
//!
//! Widgets are client-side canvas cards with no governed data: they read no
//! organization record, carry no receipt and are labeled "widget". The
//! server's part is two routes (`routes`): current weather from Open-Meteo
//! (`weather`, ported from our own `forma-generative-ui` connector, with a
//! fixture mode, APP-098 DS-65), and customization in words (`customize`),
//! where the fast model returns the widget's closed, typed state (`state`):
//! a skin, an accent and a font from fixed sets plus the widget's own
//! arguments. The model never writes CSS or markup (ADR-011 I5); in a demo
//! organization the rehearsed sentences map to fixed states without a model
//! call (DS-66). Placement and the per-browser layout are the canvas's
//! (`app/admin/src/canvas/widgets/`).
pub mod customize;
pub mod routes;
pub mod state;
pub mod weather;

#[cfg(test)]
mod tests;

/// `GET /v1/widgets/weather?location=&units=`.
pub const WEATHER_PATH: &str = "/v1/widgets/weather";
/// `POST /v1/widgets/customize`.
pub const CUSTOMIZE_PATH: &str = "/v1/widgets/customize";

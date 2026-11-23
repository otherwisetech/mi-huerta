//! Mi Huerta's core: no UI, no GPUI, so a future mobile app can reuse it.
//!
//! - [`catalog`]: the built-in plants with cited care rules (`data/*.toml`)
//! - [`model`]: the garden (plants, lawns, supplies, journal, care log, rain, sensors hook)
//! - [`schedule`]: care tasks from the rules and the log (done, skip, snooze, the rain skip)
//! - [`weather`]: Open-Meteo, opt-in, coarse location only
//! - [`store`]: the garden folder under the Otherwise data root (lock, atomic saves, Save As,
//!   Move, photos copied in, conflicted copies)
//! - [`local`]: machine-local settings (language, theme, chosen folder)
//! - [`ops`]: changes as data (clicks, and AI proposals the user accepts)
//! - [`mcp`]: the read-only MCP server with proposal cards
//! - [`feeds`]: Otherwise Apps' calendar feed and the daily reminders
pub mod catalog;
pub mod feeds;
pub mod local;
pub mod mcp;
pub mod model;
pub mod ops;
pub mod schedule;
pub mod store;
pub mod weather;

pub use catalog::Catalog;
pub use model::Garden;

pub fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

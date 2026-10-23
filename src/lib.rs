//! Open spring datasets with explicit provenance and deterministic merging.
//!
//! All HTTP requests use [`Client`] for spacing, retries, timeouts and caps.
//! Source streams yield errors: only normal exhaustion proves a complete pass.
//! No database or image downloading is provided. See the README for data licenses.

mod client;
mod merge;
mod normalize;
mod sources;
mod types;
pub mod verification;

pub use client::{Client, ClientConfig};
pub use merge::merge;
pub use normalize::{Axis, kind_from_tags, parse_coordinate, parse_coordinates, photo_url};
pub use sources::{Gnis, Noaa, Osm, Wikidata};
pub use types::*;

/// Errors fetching or decoding a source. An error means the pass is incomplete.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("HTTP status {0}")]
    Status(u16),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("delimited text: {0}")]
    Csv(#[from] csv::Error),
    #[error("ZIP: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("response exceeds {0} bytes")]
    ResponseLimit(usize),
    #[error("Overpass: {0}")]
    Overpass(String),
    #[error("invalid data: {0}")]
    Invalid(String),
}

/// Crate result type.
pub type Result<T> = std::result::Result<T, Error>;

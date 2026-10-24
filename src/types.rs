use crate::{Error, Result};
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
};

/// Source identity, ordered by precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceId {
    Osm,
    Wikidata,
    Noaa,
    Gnis,
}

/// Supported kinds. Taps are never folded into other records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Drinking,
    Spring,
    NotPotable,
    Hot,
    HotCommercial,
    Tap,
}

impl Kind {
    pub(crate) fn compatible(self, other: Self) -> bool {
        match (self, other) {
            (Self::Tap, _) | (_, Self::Tap) => false,
            (Self::Hot | Self::HotCommercial, Self::Hot | Self::HotCommercial) => true,
            (Self::Hot | Self::HotCommercial, _) | (_, Self::Hot | Self::HotCommercial) => false,
            _ => true,
        }
    }
}

/// Nullable source values. Text is limited to 40 Unicode scalars except hours
/// (80); website is a complete HTTPS URL, never truncated.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vitals {
    pub fee: Option<String>,
    pub access: Option<String>,
    pub flow: Option<String>,
    pub temperature: Option<String>,
    pub hours: Option<String>,
    pub website: Option<String>,
}

/// A normalized source row. Missing timestamps remain null, never fabricated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub kind: Kind,
    pub name: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub region: Option<String>,
    pub summary: Option<String>,
    pub vitals: Vitals,
    pub photo_url: Option<String>,
    pub source: SourceId,
    pub source_ref: String,
    pub source_url: String,
    pub source_modified_at: Option<String>,
    /// OSM's explicit Wikidata link, used before distance matching.
    #[serde(default, skip_serializing)]
    pub wikidata: Option<String>,
}

/// Attribution of one contributing source row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub source: SourceId,
    #[serde(rename = "ref")]
    pub source_ref: String,
    pub url: String,
}

/// A merged row; primary fields are flattened and provenance is primary first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spring {
    #[serde(flatten)]
    pub record: Record,
    pub cautionary: bool,
    pub sources: Vec<SourceRef>,
}

/// South, west, north, east bounds. Antimeridian bounds use west > east.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bbox {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

impl Bbox {
    pub fn new(south: f64, west: f64, north: f64, east: f64) -> Result<Self> {
        if ![south, west, north, east].iter().all(|n| n.is_finite())
            || south < -90.0
            || north > 90.0
            || south >= north
            || !(-180.0..=180.0).contains(&west)
            || !(-180.0..=180.0).contains(&east)
            || west == east
        {
            return Err(Error::Invalid(
                "bbox must be S,W,N,E within Earth ranges".into(),
            ));
        }
        Ok(Self {
            south,
            west,
            north,
            east,
        })
    }
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        (self.south..=self.north).contains(&lat)
            && if self.west <= self.east {
                (self.west..=self.east).contains(&lon)
            } else {
                lon >= self.west || lon <= self.east
            }
    }
    pub(crate) fn regular(self) -> Vec<Self> {
        if self.west < self.east {
            vec![self]
        } else {
            vec![
                Self {
                    east: 180.0,
                    ..self
                },
                Self {
                    west: -180.0,
                    ..self
                },
            ]
        }
    }
    pub(crate) fn split(self) -> [Self; 4] {
        let lat = (self.south + self.north) / 2.0;
        let lon = (self.west + self.east) / 2.0;
        [
            Self {
                north: lat,
                east: lon,
                ..self
            },
            Self {
                north: lat,
                west: lon,
                ..self
            },
            Self {
                south: lat,
                east: lon,
                ..self
            },
            Self {
                south: lat,
                west: lon,
                ..self
            },
        ]
    }
}

/// OSM tag map.
pub type Tags = BTreeMap<String, String>;
/// Source rows, followed by an error if a pass could not finish.
pub type RecordStream<'a> = Pin<Box<dyn Stream<Item = Result<Record>> + Send + 'a>>;

/// Source implementation. Streams do no work until polled.
pub trait Source: Send + Sync {
    fn id(&self) -> SourceId;
    fn fetch<'a>(&'a self, client: &'a crate::Client, progress: &'a Progress) -> RecordStream<'a>;
}

/// Shared counters. Completion means normal stream exhaustion, not a counter value.
#[derive(Debug, Default)]
pub struct Progress {
    requests: AtomicU64,
    records: AtomicU64,
    rejected: AtomicU64,
}
impl Progress {
    pub fn requests(&self) -> u64 {
        self.requests.load(Ordering::Relaxed)
    }
    pub fn records(&self) -> u64 {
        self.records.load(Ordering::Relaxed)
    }
    pub fn rejected(&self) -> u64 {
        self.rejected.load(Ordering::Relaxed)
    }
    pub(crate) fn request(&self) {
        self.requests.fetch_add(1, Ordering::Relaxed);
    }
    pub(crate) fn record(&self) {
        self.records.fetch_add(1, Ordering::Relaxed);
    }
    pub(crate) fn reject(&self) {
        self.rejected.fetch_add(1, Ordering::Relaxed);
    }
}

mod gnis;
mod noaa;
mod osm;
mod wikidata;
pub use gnis::Gnis;
pub use noaa::Noaa;
pub use osm::Osm;
pub use wikidata::Wikidata;

use crate::{Bbox, Record};
pub(crate) fn in_bounds(row: &Record, bbox: Option<Bbox>) -> bool {
    bbox.is_none_or(|b| b.contains(row.latitude, row.longitude))
}

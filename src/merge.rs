use crate::{Kind, Record, SourceId, SourceRef, Spring};
use std::collections::{BTreeMap, BTreeSet};

fn distance(a: &Record, b: &Record) -> f64 {
    let lat = (b.latitude - a.latitude).to_radians();
    let lon = (b.longitude - a.longitude).to_radians();
    let h = (lat / 2.0).sin().powi(2)
        + a.latitude.to_radians().cos() * b.latitude.to_radians().cos() * (lon / 2.0).sin().powi(2);
    6_371_008.8 * 2.0 * h.clamp(0.0, 1.0).sqrt().asin()
}

// Earth-centered 150 m cells avoid longitude/polar edge cases. Nearby points
// differ by at most one cell on each axis. Exact haversine is the final check.
fn cell(r: &Record) -> (i32, i32, i32) {
    let lat = r.latitude.to_radians();
    let lon = r.longitude.to_radians();
    let scale = 6_371_008.8 / 150.0;
    (
        (scale * lat.cos() * lon.cos()).floor() as i32,
        (scale * lat.cos() * lon.sin()).floor() as i32,
        (scale * lat.sin()).floor() as i32,
    )
}

/// Merge by OSM > Wikidata > NOAA > GNIS. Only strictly lower-precedence rows
/// fold; ties choose nearest primary then its stable source reference. An OSM
/// Wikidata tag overrides distance and kind compatibility, except for taps.
/// Output and duplicate selection are independent of input order. Matching is
/// against primary coordinates, never a transitive chain of folded locations.
pub fn merge(records: impl IntoIterator<Item = Record>) -> Vec<Spring> {
    let mut rows: Vec<_> = records
        .into_iter()
        .filter(|r| crate::normalize::valid_coordinates(r.latitude, r.longitude).is_some())
        .collect();
    rows.sort_by_cached_key(|r| {
        (
            r.source,
            r.source_ref.clone(),
            serde_json::to_string(r).expect("finite record"),
            r.wikidata.clone(),
        )
    });
    rows.dedup_by(|b, a| a.source == b.source && a.source_ref == b.source_ref);
    let mut result: Vec<Spring> = Vec::new();
    let mut grid: BTreeMap<(i32, i32, i32), Vec<usize>> = BTreeMap::new();
    let mut linked: BTreeMap<String, usize> = BTreeMap::new();
    for row in rows {
        let (x, y, z) = cell(&row);
        let explicit = if row.source == SourceId::Wikidata && row.kind != Kind::Tap {
            linked.get(&row.source_ref).copied()
        } else {
            None
        };
        let mut candidates = BTreeSet::new();
        if row.kind != Kind::Tap {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(ids) = grid.get(&(x + dx, y + dy, z + dz)) {
                            candidates.extend(ids.iter().copied());
                        }
                    }
                }
            }
        }
        let best = explicit.or_else(|| {
            candidates
                .into_iter()
                .filter_map(|i| {
                    let primary = &result[i].record;
                    if primary.source >= row.source || !primary.kind.compatible(row.kind) {
                        return None;
                    }
                    let d = distance(primary, &row);
                    (d <= 150.0).then_some((i, d))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
                .map(|(i, _)| i)
        });
        let source = SourceRef {
            source: row.source,
            source_ref: row.source_ref.clone(),
            url: row.source_url.clone(),
        };
        if let Some(i) = best {
            let spring = &mut result[i];
            let p = &mut spring.record;
            if p.name.is_none() {
                p.name = row.name;
            }
            if p.summary.is_none() {
                p.summary = row.summary;
            }
            if p.photo_url.is_none() {
                p.photo_url = row.photo_url;
            }
            if p.region.is_none() {
                p.region = row.region;
            }
            if p.vitals.temperature.is_none() {
                p.vitals.temperature = row.vitals.temperature;
            }
            spring.sources.push(source);
        } else {
            let i = result.len();
            if row.source == SourceId::Osm
                && row.kind != Kind::Tap
                && let Some(q) = &row.wikidata
            {
                linked.entry(q.clone()).or_insert(i);
            }
            grid.entry((x, y, z)).or_default().push(i);
            result.push(Spring {
                cautionary: row.kind == Kind::NotPotable,
                record: row,
                sources: vec![source],
            });
        }
    }
    result
}

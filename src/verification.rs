//! Offline fixture checks shared by the CLI and tests. Embedded in the package,
//! so `simorgh-springs verify` also works after `cargo install`.
use crate::{
    Axis, Error, Gnis, Kind, Noaa, Osm, Progress, Result, Wikidata, merge, parse_coordinate,
    parse_coordinates, photo_url,
};
use std::io::Cursor;

/// Parse the recorded source fixtures and exercise core normalization and merge.
/// Returns (check name, record/check count); never contacts a source.
pub fn verify() -> Result<Vec<(&'static str, usize)>> {
    let p = Progress::default();
    let osm = Osm::parse(include_bytes!("../fixtures/overpass.json"), &p)?;
    let baths = Osm::parse(include_bytes!("../fixtures/overpass-commercial.json"), &p)?;
    let (wikidata, bindings) = Wikidata::parse(include_bytes!("../fixtures/wikidata.json"), &p)?;
    let (commercial, _) =
        Wikidata::parse(include_bytes!("../fixtures/wikidata-commercial.json"), &p)?;
    let gnis = Gnis::parse(&include_bytes!("../fixtures/gnis.txt")[..], &p)?;
    let noaa = Noaa::parse(Cursor::new(include_bytes!("../fixtures/noaa.xlsx")), &p)?;
    let check = |ok: bool, name: &str| {
        if ok {
            Ok(())
        } else {
            Err(Error::Invalid(format!("fixture verification: {name}")))
        }
    };
    check(
        !osm.is_empty() && osm.iter().any(|r| r.kind == Kind::Tap),
        "OSM tags",
    )?;
    check(bindings == 5000 && !wikidata.is_empty(), "Wikidata page")?;
    check(
        baths.len() == 40
            && baths.iter().any(|r| {
                r.kind == Kind::HotCommercial
                    && r.source_ref.starts_with("way/")
                    && r.vitals.hours.is_some()
                    && r.vitals.website.is_some()
            }),
        "OSM commercial baths",
    )?;
    check(
        !commercial.is_empty() && commercial.iter().all(|r| r.kind == Kind::HotCommercial),
        "Wikidata commercial baths",
    )?;
    check(
        gnis.len() == 300 && gnis[0].name.as_deref() == Some("A Ninetysix Spring"),
        "GNIS rows",
    )?;
    check(
        noaa.len() == 300 && noaa[0].vitals.temperature.as_deref() == Some("92 °C"),
        "NOAA rows",
    )?;
    check(
        parse_coordinate("38° 30′ 0″ N", Axis::Latitude) == Some(38.5)
            && parse_coordinates("0", "0").is_none(),
        "coordinates",
    )?;
    check(
        photo_url("https://evil.test/pic.jpg").is_none() && photo_url("File:Spring.jpg").is_some(),
        "photo hosts",
    )?;
    let mut lower = osm
        .iter()
        .find(|r| r.kind == Kind::Spring)
        .ok_or_else(|| Error::Invalid("fixture needs an OSM spring".into()))?
        .clone();
    let primary = lower.clone();
    lower.source = crate::SourceId::Gnis;
    lower.source_ref = "fixture-merge".into();
    let merged = merge([lower, primary]);
    check(
        merged.len() == 1 && merged[0].sources.len() == 2,
        "merge precedence",
    )?;
    Ok(vec![
        ("overpass", osm.len()),
        ("wikidata", wikidata.len()),
        ("overpass_commercial", baths.len()),
        ("wikidata_commercial", commercial.len()),
        ("gnis", gnis.len()),
        ("noaa", noaa.len()),
        ("coordinates", 2),
        ("photo_hosts", 2),
        ("merge_precedence", 1),
    ])
}

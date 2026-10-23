use crate::{Kind, Tags};

/// Axis controls hemisphere validation and numeric range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Latitude,
    Longitude,
}

pub(crate) fn text(value: &str, limit: usize) -> Option<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let value: String = value
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect();
    (!value.is_empty()).then_some(value)
}

/// Map OSM tags; heat wins over potability, and drinking-water amenities are taps.
pub fn kind_from_tags(tags: &Tags) -> Option<Kind> {
    let tag = |k: &str| tags.get(k).map(String::as_str);
    if tag("amenity") == Some("drinking_water") {
        return Some(Kind::Tap);
    }
    if tag("natural") == Some("hot_spring") || tag("hot_spring") == Some("yes") {
        return Some(Kind::Hot);
    }
    if tag("natural") != Some("spring") {
        return None;
    }
    Some(match tag("drinking_water") {
        Some("yes") => Kind::Drinking,
        Some("no") => Kind::NotPotable,
        _ => Kind::Spring,
    })
}

/// Decimal (dot/comma) or DMS coordinates, including N/S/E/W/O/Ø hemispheres.
/// O and Ø mean east (Ost/Øst). Conflicting signs, wrong axes and invalid DMS fail.
pub fn parse_coordinate(input: &str, axis: Axis) -> Option<f64> {
    let cleaned: String = input
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}' | '\u{2060}'
            )
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches(',').trim().to_uppercase();
    let mut hemisphere = None;
    let mut number = String::new();
    for c in cleaned.chars() {
        if matches!(c, 'N' | 'S' | 'E' | 'W' | 'O' | 'Ø') {
            if hemisphere.replace(c).is_some() {
                return None;
            }
        } else {
            number.push(c);
        }
    }
    let sign = match (axis, hemisphere) {
        (Axis::Latitude, Some('N')) | (Axis::Longitude, Some('E' | 'O' | 'Ø')) => Some(1.0),
        (Axis::Latitude, Some('S')) | (Axis::Longitude, Some('W')) => Some(-1.0),
        (_, None) => None,
        _ => return None,
    };
    let number = number.trim().replace(',', ".");
    if number.chars().any(|c| {
        !c.is_ascii_digit()
            && !matches!(
                c,
                '.' | '-' | '+' | '°' | 'º' | '\'' | '"' | '′' | '″' | ':' | ' ' | '\t'
            )
    }) {
        return None;
    }
    let pieces: Vec<_> = number
        .split(['°', 'º', '\'', '"', '′', '″', ':', ' ', '\t'])
        .filter(|p| !p.is_empty())
        .collect();
    if pieces.is_empty() || pieces.len() > 3 {
        return None;
    }
    let degrees: f64 = pieces[0].parse().ok()?;
    let mut value = degrees.abs();
    for (i, part) in pieces.iter().enumerate().skip(1) {
        if part.starts_with(['-', '+']) {
            return None;
        }
        let n: f64 = part.parse().ok()?;
        if !(0.0..60.0).contains(&n) {
            return None;
        }
        value += n / if i == 1 { 60.0 } else { 3600.0 };
    }
    if let Some(sign) = sign {
        if degrees.is_sign_negative() && sign > 0.0 {
            return None;
        }
        value *= sign;
    } else if degrees.is_sign_negative() {
        value = -value;
    }
    let max = if axis == Axis::Latitude { 90.0 } else { 180.0 };
    (value.is_finite() && value.abs() <= max).then_some(value)
}

/// Parse and validate a coordinate pair, rejecting near-zero placeholders.
pub fn parse_coordinates(latitude: &str, longitude: &str) -> Option<(f64, f64)> {
    valid_coordinates(
        parse_coordinate(latitude, Axis::Latitude)?,
        parse_coordinate(longitude, Axis::Longitude)?,
    )
}

pub(crate) fn valid_coordinates(lat: f64, lon: f64) -> Option<(f64, f64)> {
    (lat.is_finite()
        && lon.is_finite()
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lon)
        && !(lat.abs() < 0.01 && lon.abs() < 0.01))
        .then_some((lat, lon))
}

/// Accept only HTTPS Wikimedia image/page hosts; turn a Commons File: title or
/// Wikidata P18 Special:FilePath URL into a Commons HTTPS URL. No image is fetched.
pub fn photo_url(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(file) = value.strip_prefix("File:") {
        if file.trim().is_empty() {
            return None;
        }
        let mut url = reqwest::Url::parse("https://commons.wikimedia.org/wiki/").ok()?;
        url.path_segments_mut()
            .ok()?
            .pop_if_empty()
            .push(&format!("File:{}", file.trim()));
        return Some(url.into());
    }
    let mut url = reqwest::Url::parse(value).ok()?;
    if url.host_str() == Some("commons.wikimedia.org") && url.scheme() == "http" {
        // WDQS P18 URLs conventionally use http even though Commons serves HTTPS.
        url.set_scheme("https").ok()?;
    }
    if url.scheme() != "https"
        || !matches!(
            url.host_str(),
            Some("upload.wikimedia.org" | "commons.wikimedia.org")
        )
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return None;
    }
    Some(url.into())
}

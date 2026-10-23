use crate::normalize::text;
use crate::{
    Bbox, Client, Error, Kind, Progress, Record, RecordStream, Result, Source, SourceId, Vitals,
    parse_coordinates,
};
use calamine::{Reader, Xlsx};
use std::io::{Read, Seek};

/// NOAA NCEI archived Thermal Springs List workbook (accession 0303600).
#[derive(Debug, Clone)]
pub struct Noaa {
    pub url: String,
    pub bbox: Option<Bbox>,
}
impl Default for Noaa {
    fn default() -> Self {
        Self { url:"https://www.ncei.noaa.gov/data/oceans/archive/arc0238/0303600/1.1/data/0-data/geothermal_database/data/NCEI-thermal-springs.xlsx".into(),bbox:None }
    }
}
impl Noaa {
    pub(crate) fn parse(reader: impl Read + Seek, progress: &Progress) -> Result<Vec<Record>> {
        let mut workbook =
            Xlsx::new(reader).map_err(|e| Error::Invalid(format!("NOAA workbook: {e}")))?;
        let range = workbook
            .worksheet_range_at(0)
            .ok_or_else(|| Error::Invalid("NOAA missing worksheet".into()))?
            .map_err(|e| Error::Invalid(format!("NOAA sheet: {e}")))?;
        let mut header = false;
        let mut records = Vec::new();
        for row in range.rows() {
            let get = |i| row.get(i).map(ToString::to_string).unwrap_or_default();
            let state = get(0);
            if state.starts_with("SC  LATITUDE") {
                header = true;
                continue;
            }
            if !header {
                continue;
            }
            if state.trim().is_empty() {
                continue;
            }
            if state.len() != 2 || !state.chars().all(|c| c.is_ascii_uppercase()) {
                continue;
            }
            let Some((latitude, longitude)) = parse_coordinates(&get(2), &get(4)) else {
                progress.reject();
                continue;
            };
            let raw_name = get(6);
            let name = if raw_name.eq_ignore_ascii_case("null") {
                None
            } else {
                text(&raw_name, 200)
            };
            let temperature = get(11)
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .map(|n| format!("{n} °C"));
            let state_name = match state.as_str() {
                "AK" => "Alaska",
                "AZ" => "Arizona",
                "AR" => "Arkansas",
                "CA" => "California",
                "CO" => "Colorado",
                "FL" => "Florida",
                "GA" => "Georgia",
                "HI" => "Hawaii",
                "ID" => "Idaho",
                "MT" => "Montana",
                "NV" => "Nevada",
                "NM" => "New Mexico",
                "NY" => "New York",
                "NC" => "North Carolina",
                "OR" => "Oregon",
                "SD" => "South Dakota",
                "TX" => "Texas",
                "UT" => "Utah",
                "VA" => "Virginia",
                "WA" => "Washington",
                "WV" => "West Virginia",
                "WY" => "Wyoming",
                _ => state.as_str(),
            };
            records.push(Record {
                kind: Kind::Hot,
                source: SourceId::Noaa,
                source_ref: format!("{state}|{raw_name}|{latitude}|{longitude}"),
                name,
                latitude,
                longitude,
                region: Some(format!("{state_name}, USA")),
                summary: None,
                vitals: Vitals {
                    temperature: temperature.and_then(|s| text(&s, 40)),
                    ..Vitals::default()
                },
                photo_url: None,
                source_url: "https://doi.org/10.25921/c8p0-zs06".into(),
                source_modified_at: Some("1980-01-01T00:00:00Z".into()),
                wikidata: None,
            });
        }
        if !header {
            return Err(Error::Invalid("NOAA thermal list header missing".into()));
        }
        Ok(records)
    }
}
impl Source for Noaa {
    fn id(&self) -> SourceId {
        SourceId::Noaa
    }
    fn fetch<'a>(&'a self, client: &'a Client, progress: &'a Progress) -> RecordStream<'a> {
        Box::pin(async_stream::try_stream! {
            let temp=client.download(&self.url,progress).await?;
            let rows=Self::parse(temp.reopen()?,progress)?;
            for row in rows {
                if super::in_bounds(&row,self.bbox) { progress.record();yield row; }
            }
        })
    }
}

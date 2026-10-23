use crate::normalize::text;
use crate::{
    Bbox, Client, Error, Kind, Progress, Record, RecordStream, Result, Source, SourceId, Vitals,
    parse_coordinates,
};
use std::io::Read;

/// USGS Domestic Names archive, filtered to feature_class = Spring.
#[derive(Debug, Clone)]
pub struct Gnis {
    pub url: String,
    pub bbox: Option<Bbox>,
}
impl Default for Gnis {
    fn default() -> Self {
        Self { url:"https://prd-tnm.s3.amazonaws.com/StagedProducts/GeographicNames/DomesticNames/DomesticNames_National_Text.zip".into(),bbox:None }
    }
}
impl Gnis {
    pub(crate) fn parse(reader: impl Read, progress: &Progress) -> Result<Vec<Record>> {
        let mut csv = csv::ReaderBuilder::new()
            .delimiter(b'|')
            .from_reader(reader);
        let headers = csv
            .headers()?
            .iter()
            .map(|s| s.trim_start_matches('\u{feff}').to_lowercase())
            .collect::<Vec<_>>();
        let index = |key: &str| {
            headers
                .iter()
                .position(|h| h == key)
                .ok_or_else(|| Error::Invalid(format!("GNIS missing {key}")))
        };
        let id = index("feature_id")?;
        let name = index("feature_name")?;
        let class = index("feature_class")?;
        let state = index("state_name")?;
        let county = index("county_name")?;
        let lat = index("prim_lat_dec")?;
        let lon = index("prim_long_dec")?;
        let edited = index("date_edited")?;
        let created = index("date_created")?;
        let mut rows = Vec::new();
        for row in csv.records() {
            let row = row?;
            let get = |i| row.get(i).unwrap_or("").trim();
            if get(class) != "Spring" {
                continue;
            }
            let Some((latitude, longitude)) = parse_coordinates(get(lat), get(lon)) else {
                progress.reject();
                continue;
            };
            if get(id).is_empty() {
                return Err(Error::Invalid("GNIS empty feature_id".into()));
            }
            let region = [get(county), get(state)]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            rows.push(Record {
                kind: Kind::Spring,
                name: text(get(name), 200),
                latitude,
                longitude,
                region: text(&region, 200),
                summary: None,
                vitals: Vitals::default(),
                photo_url: None,
                source: SourceId::Gnis,
                source_ref: get(id).into(),
                source_url: format!(
                    "https://edits.nationalmap.gov/apps/gaz-domestic/public/summary/{}",
                    get(id)
                ),
                source_modified_at: date(get(edited)).or_else(|| date(get(created))),
                wikidata: None,
            });
        }
        Ok(rows)
    }
}
fn date(s: &str) -> Option<String> {
    let p = s.split('/').collect::<Vec<_>>();
    if p.len() != 3 {
        return None;
    }
    let month: u8 = p[0].parse().ok()?;
    let day: u8 = p[1].parse().ok()?;
    let year: u16 = p[2].parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}T00:00:00Z"))
}
impl Source for Gnis {
    fn id(&self) -> SourceId {
        SourceId::Gnis
    }
    fn fetch<'a>(&'a self, client: &'a Client, progress: &'a Progress) -> RecordStream<'a> {
        Box::pin(async_stream::try_stream! {
            let temp=client.download(&self.url,progress).await?;
            let mut zip=zip::ZipArchive::new(temp.reopen()?)?;
            let name=zip.file_names().find(|s|s.to_ascii_lowercase().ends_with(".txt") && s.to_ascii_lowercase().contains("domesticnames")).ok_or_else(||Error::Invalid("GNIS archive missing DomesticNames text".into()))?.to_string();
            let entry=zip.by_name(&name)?;
            if entry.size()>1024*1024*1024 { Err(Error::ResponseLimit(1024*1024*1024))?; }
            let rows=Self::parse(entry,progress)?;
            for row in rows {
                if super::in_bounds(&row,self.bbox) { progress.record();yield row; }
            }
        })
    }
}

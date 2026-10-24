use crate::normalize::{text, valid_coordinates, website_url};
use crate::{
    Bbox, Client, Error, Progress, Record, RecordStream, Result, Source, SourceId, Tags, Vitals,
    kind_from_tags, photo_url,
};
use serde::Deserialize;
use std::collections::BTreeSet;

/// Overpass nodes and commercial bath ways (by centre). A 20-degree global grid
/// covers all land (and oceans).
#[derive(Debug, Clone)]
pub struct Osm {
    pub endpoints: Vec<String>,
    pub bbox: Option<Bbox>,
    /// Stop splitting rather than silently declaring an incomplete pass complete.
    pub max_split_depth: u8,
}
impl Default for Osm {
    fn default() -> Self {
        Self {
            endpoints: vec![
                "https://overpass-api.de/api/interpreter".into(),
                "https://overpass.private.coffee/api/interpreter".into(),
            ],
            bbox: None,
            max_split_depth: 10,
        }
    }
}
impl Osm {
    pub fn query(b: Bbox) -> String {
        let bbox = format!("{},{},{},{}", b.south, b.west, b.north, b.east);
        format!(
            "[out:json][timeout:300];(node[\"natural\"=\"spring\"]({bbox});node[\"natural\"=\"hot_spring\"]({bbox});node[\"amenity\"=\"drinking_water\"]({bbox});node[\"amenity\"=\"public_bath\"][\"bath:type\"~\"(^|;)[[:space:]]*(thermal|onsen|hot_spring)[[:space:]]*(;|$)\"]({bbox});way[\"amenity\"=\"public_bath\"][\"bath:type\"~\"(^|;)[[:space:]]*(thermal|onsen|hot_spring)[[:space:]]*(;|$)\"]({bbox}););out tags center meta;"
        )
    }
    fn boxes(&self) -> Vec<Bbox> {
        if let Some(b) = self.bbox {
            return b.regular();
        }
        (-90..90)
            .step_by(20)
            .flat_map(|lat| {
                (-180..180).step_by(20).map(move |lon| Bbox {
                    south: f64::from(lat),
                    north: f64::from((lat + 20).min(90)),
                    west: f64::from(lon),
                    east: f64::from(lon + 20),
                })
            })
            .collect()
    }
    async fn box_rows(
        &self,
        client: &Client,
        progress: &Progress,
        bbox: Bbox,
    ) -> Result<Vec<Record>> {
        let query = Self::query(bbox);
        let mut error = Error::Invalid("no Overpass endpoint configured".into());
        for url in &self.endpoints {
            match client.bytes(url, Some(&[("data", &query)]), progress).await {
                Ok(bytes) => return Self::parse(&bytes, progress),
                Err(e) if splittable(&e) => return Err(e),
                Err(e) => error = e,
            }
        }
        Err(error)
    }
    pub(crate) fn parse(bytes: &[u8], progress: &Progress) -> Result<Vec<Record>> {
        #[derive(Deserialize)]
        struct Response {
            elements: Vec<Node>,
            remark: Option<String>,
        }
        #[derive(Deserialize)]
        struct Node {
            #[serde(rename = "type")]
            element_type: String,
            id: u64,
            lat: Option<f64>,
            lon: Option<f64>,
            center: Option<Center>,
            #[serde(default)]
            tags: Tags,
            timestamp: Option<String>,
        }
        #[derive(Deserialize)]
        struct Center {
            lat: f64,
            lon: f64,
        }
        let data: Response = serde_json::from_slice(bytes)?;
        if let Some(remark) = data.remark {
            return Err(Error::Overpass(remark));
        }
        let mut records = Vec::new();
        for node in data.elements {
            if !matches!(node.element_type.as_str(), "node" | "way") {
                continue;
            }
            let Some(kind) = kind_from_tags(&node.tags) else {
                continue;
            };
            let center = node.center.map(|c| (c.lat, c.lon));
            let pair = if node.element_type == "way" {
                center
            } else {
                node.lat.zip(node.lon).or(center)
            };
            let Some((lat, lon)) = pair.and_then(|(lat, lon)| valid_coordinates(lat, lon)) else {
                progress.reject();
                continue;
            };
            let tag = |k: &str, n| node.tags.get(k).and_then(|s| text(s, n));
            let image = node
                .tags
                .get("wikimedia_commons")
                .and_then(|s| photo_url(s))
                .or_else(|| node.tags.get("image").and_then(|s| photo_url(s)));
            records.push(Record {
                kind,
                name: tag("name", 200),
                latitude: lat,
                longitude: lon,
                region: None,
                summary: tag("description", 280),
                vitals: Vitals {
                    fee: if node.tags.get("fee").map(String::as_str) == Some("yes") {
                        tag("charge", 40).or_else(|| tag("fee", 40))
                    } else {
                        tag("fee", 40)
                    },
                    access: tag("access", 40),
                    flow: match node.tags.get("intermittent").map(String::as_str) {
                        Some("yes") => Some("intermittent".into()),
                        Some("no") => Some("continuous".into()),
                        _ => None,
                    },
                    temperature: tag("temperature", 40),
                    hours: tag("opening_hours", 80),
                    website: node.tags.get("website").and_then(|s| website_url(s)),
                },
                photo_url: image,
                source: SourceId::Osm,
                source_ref: format!("{}/{}", node.element_type, node.id),
                source_url: format!(
                    "https://www.openstreetmap.org/{}/{}",
                    node.element_type, node.id
                ),
                source_modified_at: node.timestamp,
                wikidata: tag("wikidata", 40).filter(|s| {
                    s.starts_with('Q') && s[1..].chars().all(|c| c.is_ascii_digit()) && s.len() > 1
                }),
            });
        }
        Ok(records)
    }
}
fn splittable(e: &Error) -> bool {
    matches!(e, Error::ResponseLimit(_) | Error::Status(504))
        || matches!(e,Error::Http(e) if e.is_timeout())
        || matches!(e,Error::Overpass(s) if s.contains("timed out") || s.contains("timeout") || s.contains("out of memory") || s.contains("runtime error"))
}
impl Source for Osm {
    fn id(&self) -> SourceId {
        SourceId::Osm
    }
    fn fetch<'a>(&'a self, client: &'a Client, progress: &'a Progress) -> RecordStream<'a> {
        Box::pin(async_stream::try_stream! {
            let mut boxes:Vec<_>=self.boxes().into_iter().rev().map(|b|(b,0)).collect();
            let mut seen=BTreeSet::new();
            while let Some((bbox,depth))=boxes.pop() {
                match self.box_rows(client,progress,bbox).await {
                    Ok(rows)=>for row in rows {
                        if super::in_bounds(&row,self.bbox) && seen.insert(row.source_ref.clone()) { progress.record(); yield row; }
                    },
                    Err(e) if splittable(&e) && depth<self.max_split_depth=> {
                        boxes.extend(bbox.split().into_iter().rev().map(|b|(b,depth+1)));
                    },
                    Err(e)=>Err(e)?,
                }
            }
        })
    }
}

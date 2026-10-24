use crate::normalize::{text, valid_coordinates};
use crate::{
    Bbox, Client, Error, Kind, Progress, Record, RecordStream, Result, Source, SourceId, Vitals,
    photo_url,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// Paged WDQS source. Spring, onsen and thermal bath membership and hot
/// classification use the full subclass path.
#[derive(Debug, Clone)]
pub struct Wikidata {
    pub endpoint: String,
    pub bbox: Option<Bbox>,
    pub page_size: usize,
}
impl Default for Wikidata {
    fn default() -> Self {
        Self {
            endpoint: "https://query.wikidata.org/sparql".into(),
            bbox: None,
            page_size: 5000,
        }
    }
}
impl Wikidata {
    pub fn query(&self, offset: usize) -> String {
        format!(
            "SELECT DISTINCT ?item ?itemLabel ?itemDescription ?coord ?image ?modified ?hot ?commercial WHERE {{\n  VALUES ?class {{ wd:Q124714 wd:Q655311 wd:Q1004887 }}\n  ?item wdt:P31/wdt:P279* ?class; wdt:P625 ?coord.\n  OPTIONAL {{ ?item wdt:P18 ?image. }}\n  OPTIONAL {{ ?item schema:dateModified ?modified. }}\n  BIND(EXISTS {{ ?item wdt:P31/wdt:P279* wd:Q177380 }} AS ?hot)\n  BIND(EXISTS {{ VALUES ?bathClass {{ wd:Q655311 wd:Q1004887 }} ?item wdt:P31/wdt:P279* ?bathClass. }} AS ?commercial)\n  SERVICE wikibase:label {{ bd:serviceParam wikibase:language \"en\". }}\n}} ORDER BY ?item ?coord ?image LIMIT {} OFFSET {}",
            self.page_size, offset
        )
    }
    pub(crate) fn parse(bytes: &[u8], progress: &Progress) -> Result<(Vec<Record>, usize)> {
        #[derive(Deserialize)]
        struct Binding {
            value: String,
        }
        #[derive(Deserialize)]
        struct Results {
            bindings: Vec<BTreeMap<String, Binding>>,
        }
        #[derive(Deserialize)]
        struct Response {
            results: Results,
        }
        let data: Response = serde_json::from_slice(bytes)?;
        let count = data.results.bindings.len();
        let mut records = Vec::new();
        for row in data.results.bindings {
            let get = |key: &str| row.get(key).map(|v| v.value.as_str());
            let Some(id) = get("item").and_then(|s| s.rsplit('/').next()).filter(|s| {
                s.starts_with('Q') && s.len() > 1 && s[1..].chars().all(|c| c.is_ascii_digit())
            }) else {
                return Err(Error::Invalid("Wikidata item missing Q-id".into()));
            };
            let pair = get("coord")
                .and_then(|s| s.strip_prefix("Point("))
                .and_then(|s| s.strip_suffix(')'))
                .and_then(|s| {
                    let mut v = s.split_whitespace();
                    let lon = v.next()?.parse().ok()?;
                    let lat = v.next()?.parse().ok()?;
                    if v.next().is_some() {
                        None
                    } else {
                        valid_coordinates(lat, lon)
                    }
                });
            let Some((lat, lon)) = pair else {
                progress.reject();
                continue;
            };
            records.push(Record {
                kind: if get("commercial") == Some("true") {
                    Kind::HotCommercial
                } else if get("hot") == Some("true") {
                    Kind::Hot
                } else {
                    Kind::Spring
                },
                name: get("itemLabel")
                    .filter(|s| *s != id)
                    .and_then(|s| text(s, 200)),
                latitude: lat,
                longitude: lon,
                region: None,
                summary: get("itemDescription").and_then(|s| text(s, 280)),
                vitals: Vitals::default(),
                photo_url: get("image").and_then(photo_url),
                source: SourceId::Wikidata,
                source_ref: id.into(),
                source_url: format!("https://www.wikidata.org/wiki/{id}"),
                source_modified_at: get("modified").map(str::to_owned),
                wikidata: None,
            });
        }
        Ok((records, count))
    }
}
impl Source for Wikidata {
    fn id(&self) -> SourceId {
        SourceId::Wikidata
    }
    fn fetch<'a>(&'a self, client: &'a Client, progress: &'a Progress) -> RecordStream<'a> {
        Box::pin(async_stream::try_stream! {
            if self.page_size==0 || self.page_size>5000 { Err(Error::Invalid("Wikidata page_size must be 1..=5000".into()))?; }
            let mut offset=0;
            let mut seen=BTreeSet::new();
            loop {
                let query=self.query(offset);
                let mut url=reqwest::Url::parse(&self.endpoint).map_err(|e|Error::Invalid(e.to_string()))?;
                url.query_pairs_mut().append_pair("format","json").append_pair("query",&query);
                let bytes=client.bytes(url.as_str(),None,progress).await?;
                let (rows,count)=Self::parse(&bytes,progress)?;
                for row in rows {
                    if super::in_bounds(&row,self.bbox) && seen.insert(row.source_ref.clone()) { progress.record(); yield row; }
                }
                if count<self.page_size { break; }
                offset+=self.page_size;
            }
        })
    }
}

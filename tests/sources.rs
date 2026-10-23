use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::Response,
    routing::any,
};
use futures_util::TryStreamExt;
use simorgh_springs::*;
use std::{
    collections::VecDeque,
    io::{Cursor, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Reply {
    status: StatusCode,
    body: Vec<u8>,
    delay: Duration,
}
impl Reply {
    fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: StatusCode::OK,
            body: body.into(),
            delay: Duration::ZERO,
        }
    }
}
#[derive(Debug)]
struct Seen {
    path: String,
    query: String,
    ua: String,
    body: String,
    time: Instant,
}
struct Stub {
    url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Stub {
    fn drop(&mut self) {
        self.task.abort();
    }
}
#[derive(Clone)]
struct StubState {
    replies: Arc<Mutex<VecDeque<Reply>>>,
    seen: Arc<Mutex<Vec<Seen>>>,
}
impl Stub {
    async fn new(replies: Vec<Reply>) -> Self {
        async fn handler(State(state): State<StubState>, req: Request) -> Response {
            let uri = req.uri().clone();
            let ua = req
                .headers()
                .get("user-agent")
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
                .await
                .unwrap();
            state.seen.lock().unwrap().push(Seen {
                path: uri.path().into(),
                query: uri.query().unwrap_or_default().into(),
                ua,
                body: String::from_utf8_lossy(&body).into(),
                time: Instant::now(),
            });
            let reply = state
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected request to fixture stub");
            tokio::time::sleep(reply.delay).await;
            Response::builder()
                .status(reply.status)
                .body(Body::from(reply.body))
                .unwrap()
        }
        let state = StubState {
            replies: Arc::new(Mutex::new(replies.into())),
            seen: Arc::new(Mutex::new(Vec::new())),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = state.seen.clone();
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().fallback(any(handler)).with_state(state),
            )
            .await
            .unwrap();
        });
        Self { url, seen, task }
    }
}
fn config() -> ClientConfig {
    ClientConfig {
        min_spacing: Duration::ZERO,
        retries: 0,
        backoff: Duration::ZERO,
        timeout: Duration::from_secs(5),
        ..ClientConfig::default()
    }
}
fn client() -> Client {
    Client::new(config()).unwrap()
}
fn osm(stub: &Stub) -> Osm {
    Osm {
        endpoints: vec![format!("{}/overpass", stub.url)],
        bbox: Some(Bbox::new(38.0, -123.2, 38.9, -122.3).unwrap()),
        ..Osm::default()
    }
}
fn gnis_zip(text: &[u8]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        "Text/DomesticNames_National.txt",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(text).unwrap();
    zip.finish().unwrap().into_inner()
}
fn empty_wikidata() -> Vec<u8> {
    br#"{"results":{"bindings":[]}}"#.to_vec()
}

#[tokio::test]
async fn overpass_fixture_and_user_agent_query() {
    let stub = Stub::new(vec![Reply::ok(
        include_bytes!("../fixtures/overpass.json").to_vec(),
    )])
    .await;
    let p = Progress::default();
    let rows = osm(&stub)
        .fetch(&client(), &p)
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(rows.len(), 200);
    assert_eq!(p.records(), 200);
    assert_eq!(p.requests(), 1);
    assert_eq!(p.rejected(), 0);
    assert_eq!(rows.iter().filter(|r| r.kind == Kind::Tap).count(), 174);
    let first = &rows[0];
    assert_eq!(first.source_ref, "node/304740171");
    assert_eq!(first.latitude, 38.0152594);
    assert_eq!(
        first.source_modified_at.as_deref(),
        Some("2019-02-16T06:18:41Z")
    );
    let seen = stub.seen.lock().unwrap();
    assert_eq!(seen[0].path, "/overpass");
    assert_eq!(seen[0].ua, ClientConfig::default().user_agent);
    let decoded = reqwest::Url::parse(&format!("http://local/?{}", seen[0].body)).unwrap();
    let query = decoded
        .query_pairs()
        .find(|(k, _)| k == "data")
        .unwrap()
        .1
        .into_owned();
    assert!(query.contains("[timeout:300]"));
    assert!(query.contains("out tags center meta"));
    assert!(query.contains("38,-123.2,38.9,-122.3"));
}
#[tokio::test]
async fn wikidata_fixture_paging_modified_hot_and_photo() {
    let stub = Stub::new(vec![
        Reply::ok(include_bytes!("../fixtures/wikidata.json").to_vec()),
        Reply::ok(empty_wikidata()),
    ])
    .await;
    let source = Wikidata {
        endpoint: format!("{}/sparql", stub.url),
        ..Wikidata::default()
    };
    let p = Progress::default();
    let rows = source
        .fetch(&client(), &p)
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert!(!rows.is_empty());
    assert_eq!(rows[0].source_ref, "Q100159600");
    assert_eq!(rows[0].kind, Kind::Hot);
    assert_eq!(rows[0].latitude, 40.4487702);
    assert!(
        rows[0]
            .photo_url
            .as_ref()
            .unwrap()
            .starts_with("https://commons.wikimedia.org/")
    );
    assert_eq!(
        rows[0].source_modified_at.as_deref(),
        Some("2024-03-02T13:05:49Z")
    );
    assert_eq!(p.requests(), 2);
    let seen = stub.seen.lock().unwrap();
    for (n, r) in seen.iter().enumerate() {
        let url = reqwest::Url::parse(&format!("http://local/?{}", r.query)).unwrap();
        let q = url
            .query_pairs()
            .find(|(k, _)| k == "query")
            .unwrap()
            .1
            .into_owned();
        assert!(q.contains(&format!("LIMIT 5000 OFFSET {}", n * 5000)));
        assert!(q.contains("wdt:P31/wdt:P279* wd:Q177380"));
        assert_eq!(r.ua, ClientConfig::default().user_agent);
    }
}
#[tokio::test]
async fn gnis_zip_fixture_fields_and_date_fallback() {
    let stub = Stub::new(vec![Reply::ok(gnis_zip(include_bytes!(
        "../fixtures/gnis.txt"
    )))])
    .await;
    let p = Progress::default();
    let rows = Gnis {
        url: stub.url.clone(),
        bbox: None,
    }
    .fetch(&client(), &p)
    .try_collect::<Vec<_>>()
    .await
    .unwrap();
    assert_eq!(rows.len(), 300);
    assert_eq!(p.requests(), 1);
    assert_eq!(rows[0].source_ref, "457");
    assert_eq!(rows[0].name.as_deref(), Some("A Ninetysix Spring"));
    assert_eq!(rows[0].region.as_deref(), Some("Gila, Arizona"));
    assert_eq!(
        rows[0].source_modified_at.as_deref(),
        Some("2011-04-18T00:00:00Z")
    );
    assert_eq!(
        rows[1].source_modified_at.as_deref(),
        Some("1980-02-08T00:00:00Z")
    );
    assert!(rows.iter().all(|r| r.kind == Kind::Spring));
}
#[tokio::test]
async fn noaa_xlsx_fixture_fields_and_temperature() {
    let stub = Stub::new(vec![Reply::ok(
        include_bytes!("../fixtures/noaa.xlsx").to_vec(),
    )])
    .await;
    let p = Progress::default();
    let rows = Noaa {
        url: stub.url.clone(),
        bbox: None,
    }
    .fetch(&client(), &p)
    .try_collect::<Vec<_>>()
    .await
    .unwrap();
    assert_eq!(rows.len(), 300);
    assert_eq!(p.requests(), 1);
    let first = &rows[0];
    assert_eq!(first.name.as_deref(), Some("BAILEY HOT SPRING"));
    assert_eq!(first.region.as_deref(), Some("Alaska, USA"));
    assert_eq!(first.latitude, 55.983);
    assert_eq!(first.vitals.temperature.as_deref(), Some("92 °C"));
    assert_eq!(
        first.source_modified_at.as_deref(),
        Some("1980-01-01T00:00:00Z")
    );
    assert_eq!(first.source_url, "https://doi.org/10.25921/c8p0-zs06");
    assert!(rows.iter().all(|r| r.kind == Kind::Hot));
    assert!(rows.iter().any(|r| r.vitals.temperature.is_none()));
}
#[tokio::test]
async fn overpass_splits_on_runtime_timeout_and_deduplicates_edges() {
    let mut replies = vec![Reply::ok(
        br#"{"elements":[],"remark":"runtime error: Query timed out"}"#.to_vec(),
    )];
    replies.extend((0..4).map(|_| Reply::ok(include_bytes!("../fixtures/overpass.json").to_vec())));
    let stub = Stub::new(replies).await;
    let p = Progress::default();
    let rows = osm(&stub)
        .fetch(&client(), &p)
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(rows.len(), 200);
    assert_eq!(p.requests(), 5);
    let seen = stub.seen.lock().unwrap();
    assert_ne!(seen[0].body, seen[1].body);
    assert_ne!(seen[1].body, seen[2].body);
}
#[tokio::test]
async fn overpass_splits_on_response_cap() {
    let mut replies = vec![Reply::ok(vec![b' '; 70_000])];
    replies.extend((0..4).map(|_| Reply::ok(include_bytes!("../fixtures/overpass.json").to_vec())));
    let stub = Stub::new(replies).await;
    let c = Client::new(ClientConfig {
        response_cap: 65_000,
        ..config()
    })
    .unwrap();
    let p = Progress::default();
    let rows = osm(&stub)
        .fetch(&c, &p)
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(rows.len(), 200);
    assert_eq!(p.requests(), 5);
}
#[tokio::test]
async fn overpass_splits_on_http_timeout() {
    let mut first = Reply::ok(b"{}".to_vec());
    first.delay = Duration::from_millis(100);
    let mut replies = vec![first];
    replies.extend((0..4).map(|_| Reply::ok(b"{\"elements\":[]}".to_vec())));
    let stub = Stub::new(replies).await;
    let c = Client::new(ClientConfig {
        timeout: Duration::from_millis(30),
        ..config()
    })
    .unwrap();
    let p = Progress::default();
    let rows = osm(&stub)
        .fetch(&c, &p)
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert!(rows.is_empty());
    assert_eq!(p.requests(), 5);
}
#[tokio::test]
async fn overpass_split_limit_reports_incomplete_pass() {
    let stub = Stub::new(vec![Reply::ok(
        br#"{"elements":[],"remark":"runtime error: Query timed out"}"#.to_vec(),
    )])
    .await;
    let mut source = osm(&stub);
    source.max_split_depth = 0;
    assert!(
        source
            .fetch(&client(), &Progress::default())
            .try_collect::<Vec<_>>()
            .await
            .is_err()
    );
}
#[tokio::test]
async fn retries_use_shared_spacing_and_backoff() {
    let mut error = Reply::ok(Vec::new());
    error.status = StatusCode::TOO_MANY_REQUESTS;
    let stub = Stub::new(vec![
        error.clone(),
        error,
        Reply::ok(b"{\"elements\":[]}".to_vec()),
    ])
    .await;
    let c = Client::new(ClientConfig {
        min_spacing: Duration::from_millis(20),
        retries: 2,
        backoff: Duration::from_millis(20),
        ..config()
    })
    .unwrap();
    let p = Progress::default();
    osm(&stub)
        .fetch(&c, &p)
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(p.requests(), 3);
    let seen = stub.seen.lock().unwrap();
    assert!(seen[1].time.duration_since(seen[0].time) >= Duration::from_millis(20));
    assert!(seen[2].time.duration_since(seen[1].time) >= Duration::from_millis(40));
}
#[tokio::test]
async fn overpass_fallback_after_primary_server_error() {
    let mut error = Reply::ok(Vec::new());
    error.status = StatusCode::BAD_GATEWAY;
    let stub = Stub::new(vec![error, Reply::ok(b"{\"elements\":[]}".to_vec())]).await;
    let mut source = osm(&stub);
    source.endpoints.push(format!("{}/fallback", stub.url));
    source
        .fetch(&client(), &Progress::default())
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    let seen = stub.seen.lock().unwrap();
    assert_eq!(seen[1].path, "/fallback");
}
#[tokio::test]
async fn terminal_error_does_not_claim_complete_page() {
    let mut error = Reply::ok(Vec::new());
    error.status = StatusCode::BAD_REQUEST;
    let stub = Stub::new(vec![
        Reply::ok(include_bytes!("../fixtures/wikidata.json").to_vec()),
        error,
    ])
    .await;
    let source = Wikidata {
        endpoint: stub.url.clone(),
        ..Wikidata::default()
    };
    let c = client();
    let p = Progress::default();
    let mut stream = source.fetch(&c, &p);
    let mut count = 0;
    while let Some(row) = futures_util::StreamExt::next(&mut stream).await {
        match row {
            Ok(_) => count += 1,
            Err(Error::Status(400)) => {
                assert!(count > 0);
                return;
            }
            Err(e) => panic!("unexpected error {e}"),
        }
    }
    panic!("failed page must yield an error");
}
#[tokio::test]
async fn invalid_gnis_rows_rejected_and_non_springs_filtered() {
    let text=b"feature_id|feature_name|feature_class|state_name|county_name|prim_lat_dec|prim_long_dec|date_edited|date_created\n1|Null|Spring|CA|Napa|0|0||01/01/2000\n2|Hill|Summit|CA|Napa|38|-122||01/01/2000\n3|Good|Spring|California|Napa|38,5|-122,5||01/01/2000\n";
    let stub = Stub::new(vec![Reply::ok(gnis_zip(text))]).await;
    let p = Progress::default();
    let rows = Gnis {
        url: stub.url.clone(),
        bbox: None,
    }
    .fetch(&client(), &p)
    .try_collect::<Vec<_>>()
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(p.rejected(), 1);
    assert_eq!(rows[0].latitude, 38.5);
}
#[tokio::test]
async fn malformed_gnis_schema_fails_instead_of_empty_success() {
    let stub = Stub::new(vec![Reply::ok(gnis_zip(b"wrong|header\nfoo|bar\n"))]).await;
    assert!(
        Gnis {
            url: stub.url.clone(),
            bbox: None
        }
        .fetch(&client(), &Progress::default())
        .try_collect::<Vec<_>>()
        .await
        .is_err()
    );
}
#[tokio::test]
async fn downloads_are_capped() {
    let stub = Stub::new(vec![Reply::ok(vec![0; 1024])]).await;
    let c = Client::new(ClientConfig {
        download_cap: 100,
        ..config()
    })
    .unwrap();
    let result = Noaa {
        url: stub.url.clone(),
        bbox: None,
    }
    .fetch(&c, &Progress::default())
    .try_collect::<Vec<_>>()
    .await;
    assert!(matches!(result, Err(Error::ResponseLimit(100))));
}
#[tokio::test]
async fn bbox_filters_file_sources_after_one_download() {
    let stub = Stub::new(vec![Reply::ok(
        include_bytes!("../fixtures/noaa.xlsx").to_vec(),
    )])
    .await;
    let p = Progress::default();
    let rows = Noaa {
        url: stub.url.clone(),
        bbox: Some(Bbox::new(-90.0, 10.0, -80.0, 20.0).unwrap()),
    }
    .fetch(&client(), &p)
    .try_collect::<Vec<_>>()
    .await
    .unwrap();
    assert!(rows.is_empty());
    assert_eq!(p.requests(), 1);
}

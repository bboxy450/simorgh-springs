# simorgh-springs

Fetch, normalize and merge open spring data from OpenStreetMap, Wikidata,
NOAA NCEI and USGS GNIS. Library plus a JSON-lines CLI; no credentials,
database or image downloading. Rust 1.88+, edition 2024.

## Install and CLI

```sh
cargo install simorgh-springs --locked
simorgh-springs verify
simorgh-springs sync --out springs.jsonl --bbox '38.0,-123.2,38.9,-122.3'
simorgh-springs sync --out hot.jsonl --source noaa,wikidata
```

`sync` runs the chosen sources in precedence order and atomically replaces the
output only after all selected passes succeed. Errors exit nonzero, preserving
any existing output. Omitting `--bbox` means worldwide. Antimeridian boxes use
west > east. Wikidata pages and the NOAA/GNIS files are fetched in full and then
filtered; Overpass is queried by bounding box. `verify` repeats the embedded
fixture checks offline, including all four parsers, coordinates, photos and merge.
The full test suite also exercises fetchers through a local HTTP stub.

Each JSON line is a `Spring`: primary `Record` fields, `cautionary`, and
`sources: [{"source":"osm","ref":"node/123","url":"…"}, …]`. Source references
are stable dataset identifiers, not generated database IDs. Missing names,
regions, descriptions, photos and timestamps are null. No timestamp is invented
when a source omits it. NOAA's archive date is fixed at 1980-01-01.

## Library

```rust,no_run
use futures_util::TryStreamExt;
use simorgh_springs::{Bbox, Client, ClientConfig, Osm, Progress, Source, merge};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let client = Client::new(ClientConfig::default())?;
let source = Osm {
    bbox: Some(Bbox::new(38.0, -123.2, 38.9, -122.3)?),
    ..Osm::default()
};
let progress = Progress::default();
let records = source.fetch(&client, &progress).try_collect::<Vec<_>>().await?;
let springs = merge(records);
println!("{} springs from {} requests", springs.len(), progress.requests());
# Ok(())
# }
```

`Wikidata`, `Noaa` and `Gnis` implement the same `Source` trait. The stream yields
`Result<Record>`: only normal exhaustion proves a complete pass. Consumers must
not withdraw missing data after an error or early cancellation. Request, record
and rejected-coordinate counters are exposed by `Progress`.

`ClientConfig` defaults to the User-Agent
`simorgh-springs (+https://github.com/bboxy450/simorgh-springs)`, at least two
seconds between requests, three retries after the initial attempt with 2/4/8 s
backoff, a 360 s per-request timeout including the body, a 64 MiB JSON response
cap and a 512 MiB download cap. Clones share spacing. HTTP 408/429/5xx and
transient network failures are retried. Redirects are rejected so a redirect
cannot bypass request spacing. Endpoint URLs and policies are configurable for
consumers and local tests. Retain the two-second minimum against live services.

Overpass uses a fixed 20-degree global grid covering all land (also querying
ocean cells), with `out tags center meta` and a 300 s query timeout. Timeouts,
runtime resource errors and oversized responses split a box into four, to a
maximum depth of ten; exhaustion yields an error, never a false complete pass.
The secondary Overpass endpoint handles other primary endpoint failures.
Wikidata uses stable ordered `LIMIT 5000 OFFSET n` pages and subclass paths for
spring, onsen (Q655311) and thermal bath (Q1004887) membership and hot-spring
classification. NOAA and GNIS each download
once per successful pass into an automatically deleted temporary file, then
parse the archived XLSX or ZIP/pipe-delimited text. Retries can repeat a failed
download. GNIS ZIP entries are limited to 1 GiB uncompressed.

## Normalization and merge

Kinds are `drinking`, `spring`, `not_potable`, `hot`, `hot_commercial`, `tap`. OSM drinking-water
amenities are taps; heat takes precedence over potability on natural springs.
Public baths with a `bath:type` value of `thermal`, `onsen` or `hot_spring`
(including semicolon-separated values) are commercial hot springs; both nodes
and ways are queried, with ways located by their centre. A natural hot spring
stays `hot` even with a fee or public-bath tags. Generic spas are excluded.
Only `not_potable` sets `cautionary`. GNIS rows are springs, NOAA rows are hot;
Wikidata onsen/thermal bath subclasses are commercial, other hot subclasses
are hot and other springs have unknown potability.

Names trim to 200 Unicode scalars, summaries to 280, and text vitals to 40
except `hours` (80). Vitals contain `fee`, `access`, `flow`, `temperature`,
`hours` and `website`. OSM `opening_hours` supplies hours; when `fee=yes`,
nonempty `charge` supplies the fee text, otherwise the original fee is kept.
Website accepts only complete HTTPS URLs without credentials and is never
truncated. Missing values are null. NOAA's nonnumeric B/H/W
temperature codes remain null. Coordinates accept decimal comma/dot, DMS,
N/S/E/W/O/Ø (Ost/Øst = east), zero-width cleanup and trailing commas. Wrong axes,
invalid ranges, nonfinite numbers and pairs with both magnitudes below 0.01
are rejected. Photos allow only HTTPS `upload.wikimedia.org` or
`commons.wikimedia.org`; Commons `File:` titles and WDQS P18 HTTP Commons URLs
are converted to HTTPS Commons links. Image bytes are never requested.

`merge` sorts by `osm > wikidata > noaa > gnis`, then source reference, with a
canonical tie-break for duplicate source IDs. A strictly lower-precedence row
within 150 m (great-circle distance) folds into the nearest compatible primary:
spring/drinking/not_potable together, hot/hot_commercial together. Equal-distance ties use the
primary's stable order. Same-source rows remain separate. An OSM `wikidata`
link folds that item regardless of distance or kind, except that taps **never**
merge. Matching uses primary coordinates, without transitive chains.

Only missing name, summary, photo, region and temperature are filled. Primary
kind, coordinates, timestamps, access, fee, hours and website stay primary. All contributing
references appear in precedence order, primary first. The same input records
produce the same output regardless of input order.

## Sources, data licenses and required attribution

The **code** is licensed under Apache-2.0; see [LICENSE](LICENSE). The source
datasets and merged output have separate licenses:

| Dataset | Data license | Source |
| --- | --- | --- |
| OpenStreetMap | ODbL 1.0 | [Overpass](https://overpass-api.de/api/interpreter), [copyright](https://www.openstreetmap.org/copyright) |
| Wikidata | CC0 | [SPARQL](https://query.wikidata.org/sparql), [data access/license](https://www.wikidata.org/wiki/Wikidata:Licensing) |
| NOAA NCEI Thermal Springs List | CC0 | [Accession 0303600](https://doi.org/10.25921/c8p0-zs06), [archived workbook](https://www.ncei.noaa.gov/data/oceans/archive/arc0238/0303600/1.1/data/0-data/geothermal_database/data/NCEI-thermal-springs.xlsx) |
| USGS GNIS Domestic Names | Public domain | [Domestic Names ZIP](https://prd-tnm.s3.amazonaws.com/StagedProducts/GeographicNames/DomesticNames/DomesticNames_National_Text.zip), [USGS GNIS](https://www.usgs.gov/tools/geographic-names-information-system-gnis) |

Consumers must show the contributing sources and links, including
**“© OpenStreetMap contributors, ODbL”**, linked to
<https://www.openstreetmap.org/copyright>. Acknowledge the U.S. Geological
Survey: **“Geographic names data from the U.S. Geological Survey GNIS.”** Name
**“NOAA NCEI (CC0)”** and **“Wikidata (CC0)”**. Suggested combined footer:

> Map data © OpenStreetMap contributors (ODbL) · USGS GNIS · NOAA NCEI · Wikidata

When output contains OSM records, the merged output is an **ODbL derivative
database**. Offer the extracted database under ODbL and make it available on
request; publishing this Apache-2.0 code does not replace the data license.
Preserve `sources` so downstream users can attribute every contribution. Linked
Wikimedia photographs have their own licenses on their file pages.

The layer is not a water-quality assessment. Consumers should display
“Water quality isn't verified. Check before drinking.” and present
`not_potable` records as **Not for drinking**.

## Verification

All fixture captures and exact URLs are in [fixtures/README.md](fixtures/README.md).
Tests use recorded data and loopback HTTP only; no live-source access.

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo doc --no-deps
cargo run --locked -- verify
cargo publish --dry-run
```

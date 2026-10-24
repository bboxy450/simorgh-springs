# Recorded source fixtures

Initial fixtures captured once on 2026-10-23 UTC with User-Agent
`simorgh-springs (+https://github.com/bboxy450/simorgh-springs)`.
All requests, including discovery, were spaced by at least two seconds.
No source is contacted by the tests or CLI `verify`.

- `overpass.json`: unmodified 200-node response, Sonoma/Napa bounds
  (38.0, -123.2, 38.9, -122.3); 26 springs and 174 drinking-water points.
  POST to `https://overpass-api.de/api/interpreter`, URL-encoded form field
  `data` with the exact query in `overpass-query.txt`.
- `wikidata.json`: unmodified first SPARQL page, 5,000 bindings; duplicate
  item bindings are possible. Exact query in `wikidata-query.rq`.
  Exact GET URL:

  `https://query.wikidata.org/sparql?format=json&query=SELECT+%3Fitem+%3FitemLabel+%3FitemDescription+%3Fcoord+%3Fimage+%3Fmodified+%3Fhot+WHERE+%7B%0A++%3Fitem+wdt%3AP31%2Fwdt%3AP279%2A+wd%3AQ124714%3B+wdt%3AP625+%3Fcoord.%0A++OPTIONAL+%7B+%3Fitem+wdt%3AP18+%3Fimage.+%7D%0A++OPTIONAL+%7B+%3Fitem+schema%3AdateModified+%3Fmodified.+%7D%0A++BIND%28EXISTS+%7B+%3Fitem+wdt%3AP31%2Fwdt%3AP279%2A+wd%3AQ177380+%7D+AS+%3Fhot%29%0A++SERVICE+wikibase%3Alabel+%7B+bd%3AserviceParam+wikibase%3Alanguage+%22en%22.+%7D%0A%7D+ORDER+BY+%3Fitem+%3Fcoord+%3Fimage+LIMIT+5000+OFFSET+0`

- `gnis.txt`: original header and first 300 `feature_class=Spring` rows,
  in source order, from `Text/DomesticNames_National.txt` inside:
  `https://prd-tnm.s3.amazonaws.com/StagedProducts/GeographicNames/DomesticNames/DomesticNames_National_Text.zip`.
  UTF-8 text, original columns and values, line endings normalized to LF.
- `noaa.xlsx`: first five descriptive/header rows and first 300 data rows
  from the first sheet. Remaining sheet rows removed, dimension adjusted
  to A1:Z305; workbook members, shared strings and source values retained.
  Exact source:
  `https://www.ncei.noaa.gov/data/oceans/archive/arc0238/0303600/1.1/data/0-data/geothermal_database/data/NCEI-thermal-springs.xlsx`.
  The archival workbook stores the source columns in A (state code), C
  (latitude), E (longitude), G (name) and L (maximum temperature °C).

NOAA discovery (each retrieved once):

- `https://www.ncei.noaa.gov/data/oceans/archive/arc0238/0303600/1.1/data/0-data/geothermal_database/`
- `https://www.ncei.noaa.gov/data/oceans/archive/arc0238/0303600/1.1/data/0-data/geothermal_database/README.txt`
- `https://www.ncei.noaa.gov/data/oceans/archive/arc0238/0303600/1.1/data/0-data/geothermal_database/data/`

## 0.2.0 commercial bath extension

Captured once on 2026-10-24 UTC using the same declared User-Agent and no
retries or discovery requests. Request starts were 89.431 seconds apart:

- `overpass-commercial.json`: unmodified HTTP 200 response, 36,992 bytes;
  request started at 15:39:04 UTC. The global sample contains 20 nodes and
  20 ways, including all three bath types, fees, hours, HTTPS and HTTP
  websites. Two natural hot springs remain `hot`; the other 38 rows are
  `hot_commercial`. Way coordinates come from the returned `center`.
  POST to `https://overpass-api.de/api/interpreter`, URL-encoded form field
  `data` with the exact query in `overpass-commercial-query.txt`. The query
  limits nodes and ways separately to 20 each.
- `wikidata-commercial.json`: unmodified HTTP 200 response, 92,491 bytes;
  request started at 15:40:33 UTC. The 100 bindings contain 99 distinct
  items, including onsen and thermal baths, with both hot and non-hot
  classifications. Exact query in `wikidata-commercial-query.rq`: the
  extended membership/classification query filtered to commercial rows,
  with a 100-binding limit. Exact GET URL:

  `https://query.wikidata.org/sparql?format=json&query=SELECT%20DISTINCT%20%3Fitem%20%3FitemLabel%20%3FitemDescription%20%3Fcoord%20%3Fimage%20%3Fmodified%20%3Fhot%20%3Fcommercial%20WHERE%20%7B%0A%20%20VALUES%20%3Fclass%20%7B%20wd%3AQ124714%20wd%3AQ655311%20wd%3AQ1004887%20%7D%0A%20%20%3Fitem%20wdt%3AP31%2Fwdt%3AP279%2A%20%3Fclass%3B%20wdt%3AP625%20%3Fcoord.%0A%20%20OPTIONAL%20%7B%20%3Fitem%20wdt%3AP18%20%3Fimage.%20%7D%0A%20%20OPTIONAL%20%7B%20%3Fitem%20schema%3AdateModified%20%3Fmodified.%20%7D%0A%20%20BIND%28EXISTS%20%7B%20%3Fitem%20wdt%3AP31%2Fwdt%3AP279%2A%20wd%3AQ177380%20%7D%20AS%20%3Fhot%29%0A%20%20BIND%28EXISTS%20%7B%20VALUES%20%3FbathClass%20%7B%20wd%3AQ655311%20wd%3AQ1004887%20%7D%20%3Fitem%20wdt%3AP31%2Fwdt%3AP279%2A%20%3FbathClass.%20%7D%20AS%20%3Fcommercial%29%0A%20%20FILTER%28%3Fcommercial%29%0A%20%20SERVICE%20wikibase%3Alabel%20%7B%20bd%3AserviceParam%20wikibase%3Alanguage%20%22en%22.%20%7D%0A%7D%20ORDER%20BY%20%3Fitem%20%3Fcoord%20%3Fimage%20LIMIT%20100%20OFFSET%200%0A`

The original four fixtures and their capture queries remain unchanged.
Tests and CLI `verify` read the new captures offline; edge cases use
synthetic payloads served only by a loopback HTTP stub.

Data licenses: OpenStreetMap ODbL 1.0; Wikidata CC0; NOAA CC0;
USGS GNIS public domain. These data are not relicensed by the code's
Apache-2.0 license. See the repository README for attribution and ODbL
share-alike obligations.

SHA-256 of retained files:

```text
85068e78d42c0b81f23f4be55df4e56af56ebbef40e23ffaefb669cadf7489c3  gnis.txt
32b53cd58e39c933036d29617f454f35c0a5a3bc18f623626fc145ff1281e00d  noaa.xlsx
5d3f82f2ad56bf8f50ff6571f30d8d552e59bfade72d899d0fef297f6f614c13  overpass-query.txt
e0190716ee639915b6feb84d7925793c755102bd698575848a8945327be7cdba  overpass.json
b30d9c734f07498d65dbb9e962b8ccf3eee1a49a2366d7e8077df0a85680080d  wikidata-query.rq
b8ce2cab93a27ceb16ed7db6a58190609787344b7ad0f80089de81f2732e56a8  wikidata.json
90c5ec92930c6623bb12f134cd3cf3e4b156357bfed5bbcc1360c68ac995e65e  overpass-commercial-query.txt
349d98f9f3aabb435a1fc28c1aed559a273fe9749e62243cbf0ba0b5a8fb0d03  overpass-commercial.json
1c78f402090751e51dbc0879a77fd78ce90564f64a8586ad0204dc1cb30af1f9  wikidata-commercial-query.rq
32f2b662021cfed75d01dc0ed31e29e0d11b1b2aeb6a4f5cb5de6b10e5610d0d  wikidata-commercial.json
```

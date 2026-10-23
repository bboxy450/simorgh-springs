# Recorded source fixtures

Captured once on 2026-10-23 UTC with User-Agent
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
```

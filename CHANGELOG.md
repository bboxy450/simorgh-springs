# Changelog

## 0.2.0 — 2026-10-24

- Add `hot_commercial` for OSM thermal/onsen/hot-spring public baths and
  Wikidata onsen (Q655311) / thermal bath (Q1004887) subclasses.
- Fetch public-bath nodes and ways from Overpass, using way centres and stable
  `way/…` references. Natural hot springs remain `hot`, including paid springs.
- Fold commercial and natural hot springs across sources within 150 m while
  keeping same-source baths separate and preserving primary vitals.
- Add nullable `hours` (80 Unicode scalars) and HTTPS `website` vitals; use
  OSM `charge` as fee text when `fee=yes`.
- Extend recorded live-source fixtures and offline normalization, parser,
  query, merge and JSON contract checks.

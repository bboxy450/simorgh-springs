use simorgh_springs::*;

fn record(source: SourceId, id: &str, kind: Kind, lat: f64, lon: f64) -> Record {
    Record {
        source,
        source_ref: id.into(),
        kind,
        latitude: lat,
        longitude: lon,
        name: None,
        region: None,
        summary: None,
        vitals: Vitals::default(),
        photo_url: None,
        source_url: format!("https://example.test/{id}"),
        source_modified_at: None,
        wikidata: None,
    }
}
#[test]
fn tag_to_kind_mapping() {
    for (tags, want) in [
        (vec![("natural", "spring")], Some(Kind::Spring)),
        (
            vec![("natural", "spring"), ("drinking_water", "yes")],
            Some(Kind::Drinking),
        ),
        (
            vec![("natural", "spring"), ("drinking_water", "no")],
            Some(Kind::NotPotable),
        ),
        (vec![("natural", "hot_spring")], Some(Kind::Hot)),
        (
            vec![
                ("natural", "spring"),
                ("hot_spring", "yes"),
                ("drinking_water", "no"),
            ],
            Some(Kind::Hot),
        ),
        (
            vec![("amenity", "drinking_water"), ("drinking_water", "yes")],
            Some(Kind::Tap),
        ),
        (vec![("natural", "water")], None),
    ] {
        assert_eq!(
            kind_from_tags(
                &tags
                    .into_iter()
                    .map(|(k, v)| (k.into(), v.into()))
                    .collect()
            ),
            want
        );
    }
}
#[test]
fn commercial_bath_tags_and_natural_hot_precedence() {
    for bath_type in ["thermal", "onsen", "hot_spring", "foot_bath; onsen ;cold"] {
        let mut tags = Tags::from([
            ("amenity".into(), "public_bath".into()),
            ("bath:type".into(), bath_type.into()),
            ("fee".into(), "yes".into()),
        ]);
        assert_eq!(kind_from_tags(&tags), Some(Kind::HotCommercial));
        tags.insert("natural".into(), "hot_spring".into());
        assert_eq!(kind_from_tags(&tags), Some(Kind::Hot));
    }
    for (amenity, bath_type) in [
        ("public_bath", "cold"),
        ("public_bath", ""),
        ("public_bath", "nonthermal"),
        ("spa", "thermal"),
        ("", "onsen"),
    ] {
        let tags = Tags::from([
            ("amenity".into(), amenity.into()),
            ("bath:type".into(), bath_type.into()),
        ]);
        assert_eq!(kind_from_tags(&tags), None);
    }
    assert_eq!(
        kind_from_tags(&Tags::from([("leisure".into(), "spa".into())])),
        None
    );
    assert_eq!(
        kind_from_tags(&Tags::from([
            ("natural".into(), "hot_spring".into()),
            ("fee".into(), "yes".into()),
        ])),
        Some(Kind::Hot)
    );
}

#[test]
fn commercial_hot_merge_compatibility_in_both_directions() {
    for kind in [
        Kind::Hot,
        Kind::HotCommercial,
        Kind::Spring,
        Kind::Drinking,
        Kind::NotPotable,
        Kind::Tap,
    ] {
        for (primary, lower) in [(Kind::HotCommercial, kind), (kind, Kind::HotCommercial)] {
            let rows = merge([
                record(SourceId::Osm, "node/1", primary, 38.0, -122.0),
                record(SourceId::Wikidata, "Q1", lower, 38.0001, -122.0),
            ]);
            let compatible = matches!(kind, Kind::Hot | Kind::HotCommercial);
            assert_eq!(rows.len(), if compatible { 1 } else { 2 });
            assert_eq!(rows[0].record.kind, primary);
            assert_eq!(rows[0].cautionary, primary == Kind::NotPotable);
        }
    }
}

#[test]
fn commercial_150_m_boundary_and_same_source_separation() {
    let degree = 150.0 / 6_371_008.8 * 180.0 / std::f64::consts::PI;
    let natural = record(SourceId::Osm, "node/1", Kind::Hot, 38.0, -122.0);
    let rows = merge([
        natural.clone(),
        record(
            SourceId::Wikidata,
            "Q1",
            Kind::HotCommercial,
            38.0 + degree * 0.999,
            -122.0,
        ),
        record(
            SourceId::Wikidata,
            "Q2",
            Kind::HotCommercial,
            38.0 + degree * 1.001,
            -122.0,
        ),
    ]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].sources.len(), 2);
    let rows = merge([
        natural,
        record(
            SourceId::Osm,
            "node/2",
            Kind::HotCommercial,
            38.0001,
            -122.0,
        ),
        record(SourceId::Osm, "way/1", Kind::HotCommercial, 38.0001, -122.0),
    ]);
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| r.sources.len() == 1));
}

#[test]
fn commercial_kind_and_vitals_json_contract_is_additive() {
    let old: Vitals = serde_json::from_value(serde_json::json!({
        "fee": "yes", "access": null, "flow": null, "temperature": null
    }))
    .unwrap();
    assert!(old.hours.is_none());
    assert!(old.website.is_none());
    let mut row = record(SourceId::Osm, "way/1", Kind::HotCommercial, 38.0, -122.0);
    row.vitals.hours = Some("Mo-Su 09:00-21:00".into());
    row.vitals.website = Some("https://example.test/bath".into());
    let json = serde_json::to_value(&row).unwrap();
    assert_eq!(json["kind"], "hot_commercial");
    assert_eq!(json["vitals"]["hours"], "Mo-Su 09:00-21:00");
    assert_eq!(json["vitals"]["website"], "https://example.test/bath");
    assert_eq!(serde_json::from_value::<Record>(json).unwrap(), row);
}

#[test]
fn merge_keeps_primary_commercial_vitals_even_when_missing() {
    for present in [false, true] {
        let mut primary = record(SourceId::Osm, "way/1", Kind::HotCommercial, 38.0, -122.0);
        if present {
            primary.vitals.hours = Some("09:00-21:00".into());
            primary.vitals.website = Some("https://primary.test/".into());
            primary.vitals.fee = Some("20 EUR".into());
        }
        let mut lower = record(SourceId::Wikidata, "Q1", Kind::HotCommercial, 38.0, -122.0);
        lower.vitals.hours = Some("24/7".into());
        lower.vitals.website = Some("https://lower.test/".into());
        lower.vitals.fee = Some("yes".into());
        let expected = primary.vitals.clone();
        let rows = merge([lower, primary]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].record.vitals, expected);
    }
}
#[test]
fn decimal_comma_dot_zero_width_and_trailing_comma() {
    for s in ["38.5", "38,5", "\u{feff}38\u{200b}.5,", " 38,5 N, "] {
        assert_eq!(parse_coordinate(s, Axis::Latitude), Some(38.5), "{s}");
    }
}
#[test]
fn hemisphere_letters_and_dms() {
    for (s, axis, want) in [
        ("38° 30′ 0″ N", Axis::Latitude, 38.5),
        ("38 30 0 S", Axis::Latitude, -38.5),
        ("122° 15' 0\" W", Axis::Longitude, -122.25),
        ("122,25 E", Axis::Longitude, 122.25),
        ("122.25 O", Axis::Longitude, 122.25),
        ("122.25 Ø", Axis::Longitude, 122.25),
        ("-122.25W", Axis::Longitude, -122.25),
    ] {
        assert_eq!(parse_coordinate(s, axis), Some(want), "{s}");
    }
}
#[test]
fn coordinates_reject_invalid_range_axis_and_dms() {
    for s in [
        "91",
        "-90.1",
        "NaN",
        "inf",
        "38° 60' N",
        "38 12 60 N",
        "38 NN",
        "-38 N",
        "38 E",
        "38/12",
        "north",
        "38foo",
        "38.5.1",
    ] {
        assert!(parse_coordinate(s, Axis::Latitude).is_none(), "{s}");
    }
    assert!(parse_coordinate("181", Axis::Longitude).is_none());
    assert!(parse_coordinates("0.001", "-0.009").is_none());
    assert_eq!(parse_coordinates("0", "0.01"), Some((0.0, 0.01)));
    assert_eq!(parse_coordinates("90", "180"), Some((90.0, 180.0)));
}
#[test]
fn wikimedia_photo_host_allowlist() {
    for s in [
        "https://evil.test/a",
        "https://upload.wikimedia.org.evil.test/a",
        "https://commons.wikimedia.org@evil.test/a",
        "http://upload.wikimedia.org/a",
        "https://user@commons.wikimedia.org/a",
        "https://commons.wikimedia.org:444/a",
        "javascript:alert(1)",
        "File:",
        "Category:Springs",
    ] {
        assert!(photo_url(s).is_none(), "{s}");
    }
    assert_eq!(
        photo_url("http://commons.wikimedia.org/wiki/Special:FilePath/A.jpg").unwrap(),
        "https://commons.wikimedia.org/wiki/Special:FilePath/A.jpg"
    );
    assert_eq!(
        photo_url("File:A #é.jpg").unwrap(),
        "https://commons.wikimedia.org/wiki/File:A%20%23%C3%A9.jpg"
    );
    assert!(photo_url("https://upload.wikimedia.org/a.jpg").is_some());
}
#[test]
fn merge_precedence_and_missing_fields_only() {
    let mut osm = record(SourceId::Osm, "node/1", Kind::NotPotable, 38.0, -122.0);
    osm.name = Some("OSM name".into());
    osm.vitals.access = Some("private".into());
    let mut wd = record(SourceId::Wikidata, "Q1", Kind::Drinking, 38.0001, -122.0);
    wd.name = Some("WD name".into());
    wd.summary = Some("WD summary".into());
    wd.vitals.fee = Some("yes".into());
    let mut noaa = record(SourceId::Noaa, "N1", Kind::Spring, 38.0002, -122.0);
    noaa.region = Some("region".into());
    noaa.vitals.temperature = Some("20 °C".into());
    let mut gnis = record(SourceId::Gnis, "G1", Kind::Spring, 38.0003, -122.0);
    gnis.summary = Some("GNIS summary".into());
    gnis.photo_url = Some("https://commons.wikimedia.org/a".into());
    let out = merge([gnis, noaa, wd, osm]);
    assert_eq!(out.len(), 1);
    let s = &out[0];
    assert_eq!(s.record.source, SourceId::Osm);
    assert!(s.cautionary);
    assert_eq!(s.record.name.as_deref(), Some("OSM name"));
    assert_eq!(s.record.summary.as_deref(), Some("WD summary"));
    assert_eq!(s.record.region.as_deref(), Some("region"));
    assert!(s.record.photo_url.is_some());
    assert_eq!(s.record.vitals.temperature.as_deref(), Some("20 °C"));
    assert!(s.record.vitals.fee.is_none());
    assert_eq!(
        s.sources.iter().map(|r| r.source).collect::<Vec<_>>(),
        [
            SourceId::Osm,
            SourceId::Wikidata,
            SourceId::Noaa,
            SourceId::Gnis
        ]
    );
}
#[test]
fn merge_150_m_boundary_and_no_transitive_fold() {
    let degree = 150.0 / 6_371_008.8 * 180.0 / std::f64::consts::PI;
    let osm = record(SourceId::Osm, "node/1", Kind::Spring, 38.0, -122.0);
    let near = record(
        SourceId::Wikidata,
        "Q1",
        Kind::Spring,
        38.0 + degree * 0.999,
        -122.0,
    );
    let far = record(
        SourceId::Gnis,
        "G1",
        Kind::Spring,
        38.0 + degree * 1.001,
        -122.0,
    );
    let out = merge([far, near, osm]);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].sources.len(), 2);
}
#[test]
fn hot_and_spring_are_incompatible() {
    let out = merge([
        record(SourceId::Osm, "node/1", Kind::Hot, 38.0, -122.0),
        record(SourceId::Wikidata, "Q1", Kind::Spring, 38.0, -122.0),
        record(SourceId::Noaa, "N1", Kind::Hot, 38.0, -122.0),
    ]);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].sources.len(), 2);
}
#[test]
fn osm_wikidata_shortcut_overrides_distance_and_kind() {
    let mut osm = record(SourceId::Osm, "node/1", Kind::Spring, 38.0, -122.0);
    osm.wikidata = Some("Q1".into());
    let wd = record(SourceId::Wikidata, "Q1", Kind::Hot, -38.0, 122.0);
    let out = merge([wd, osm]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].sources.len(), 2);
}
#[test]
fn taps_never_merge_even_with_wikidata_link() {
    let mut osm = record(SourceId::Osm, "node/1", Kind::Tap, 38.0, -122.0);
    osm.wikidata = Some("Q1".into());
    let out = merge([
        osm,
        record(SourceId::Wikidata, "Q1", Kind::Spring, 38.0, -122.0),
        record(SourceId::Gnis, "G1", Kind::Tap, 38.0, -122.0),
    ]);
    assert_eq!(out.len(), 3);
    assert!(out.iter().all(|s| s.sources.len() == 1));
}
#[test]
fn same_source_never_folds_and_duplicates_are_deterministic() {
    let a = record(SourceId::Osm, "node/1", Kind::Spring, 38.0, -122.0);
    let b = record(SourceId::Osm, "node/2", Kind::Spring, 38.0, -122.0);
    let mut duplicate = a.clone();
    duplicate.name = Some("alternate".into());
    let wd = record(SourceId::Wikidata, "Q1", Kind::Spring, 38.0, -122.0);
    let rows = vec![duplicate, a, b, wd];
    let expected = merge(rows.clone());
    assert_eq!(expected.len(), 2);
    assert_eq!(expected[0].sources.len(), 2);
    for n in 0..rows.len() {
        let mut perm = rows.clone();
        perm.rotate_left(n);
        assert_eq!(merge(perm.clone()), expected);
        perm.reverse();
        assert_eq!(merge(perm), expected);
    }
}
#[test]
fn merge_handles_antimeridian_and_poles() {
    let out = merge([
        record(SourceId::Osm, "node/1", Kind::Spring, 10.0, 179.9999),
        record(SourceId::Gnis, "1", Kind::Spring, 10.0, -179.9999),
        record(SourceId::Osm, "node/2", Kind::Hot, 89.9999, 0.0),
        record(SourceId::Noaa, "2", Kind::Hot, 89.9999, 180.0),
    ]);
    assert_eq!(out.len(), 2);
    assert!(out.iter().all(|s| s.sources.len() == 2));
}
#[test]
fn invalid_coordinates_do_not_enter_merge() {
    assert!(
        merge([
            record(SourceId::Gnis, "1", Kind::Spring, 0.0, 0.0),
            record(SourceId::Gnis, "2", Kind::Spring, f64::NAN, 1.0)
        ])
        .is_empty()
    );
}
#[test]
fn bbox_validation_and_antimeridian() {
    assert!(Bbox::new(10.0, 0.0, -10.0, 1.0).is_err());
    assert!(Bbox::new(0.0, f64::NAN, 10.0, 1.0).is_err());
    let b = Bbox::new(-10.0, 170.0, 10.0, -170.0).unwrap();
    assert!(b.contains(0.0, 179.0));
    assert!(b.contains(0.0, -179.0));
    assert!(!b.contains(0.0, 0.0));
}
#[test]
fn embedded_verifier_is_offline() {
    assert_eq!(verification::verify().unwrap().len(), 9);
}
#[test]
fn cli_verify_runs_embedded_fixtures() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_simorgh-springs"))
        .arg("verify")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("noaa: ok (300)"));
}
#[test]
fn cli_rejects_invalid_source_and_bbox() {
    for args in [
        vec!["sync", "--out", "unused.jsonl", "--source", "unknown"],
        vec!["sync", "--out", "unused.jsonl", "--bbox", "90,0,-90,1"],
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_simorgh-springs"))
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
}

#[test]
fn merged_json_preserves_provenance_without_internal_link_metadata() {
    let mut osm = record(SourceId::Osm, "node/1", Kind::Spring, 38.0, -122.0);
    osm.wikidata = Some("Q1".into());
    let merged = merge([
        osm,
        record(SourceId::Wikidata, "Q1", Kind::Spring, -38.0, 122.0),
    ]);
    let json = serde_json::to_value(&merged[0]).unwrap();
    assert!(json.get("wikidata").is_none());
    assert_eq!(json["sources"][1]["ref"], "Q1");
    assert_eq!(json["sources"][0]["source"], "osm");
}

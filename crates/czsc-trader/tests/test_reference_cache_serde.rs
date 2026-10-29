//! Old positional snapshots must remain readable after appending reference facts.
use czsc_core::objects::state::MacdSeries;
use czsc_signals::types::{BollSeries, KdjSeries, TaCache};
use serde::Serialize;
use std::collections::HashMap;

// Exact pre-reference field order. `updated_macd_keys` was serde(skip).
#[derive(Serialize)]
struct LegacyTaCache {
    series: HashMap<String, Vec<f64>>,
    macd: HashMap<String, MacdSeries>,
    boll: HashMap<String, BollSeries>,
    boll_ids: HashMap<String, Vec<i32>>,
    kdj: HashMap<String, KdjSeries>,
    series_ids: HashMap<String, Vec<i32>>,
    last_len: usize,
}

#[test]
fn legacy_json_and_positional_messagepack_preserve_all_fields() {
    let old = LegacyTaCache {
        series: [("SMA#2".into(), vec![11.0, 12.0])].into(),
        macd: [(
            "MACD".into(),
            MacdSeries {
                ids: vec![22],
                dif: vec![0.1],
                dea: vec![0.2],
                macd: vec![-0.2],
            },
        )]
        .into(),
        boll: [(
            "BOLL".into(),
            BollSeries {
                upper: vec![13.0],
                mid: vec![12.0],
                lower: vec![11.0],
            },
        )]
        .into(),
        boll_ids: [("BOLL".into(), vec![24])].into(),
        kdj: [(
            "KDJ".into(),
            KdjSeries {
                ids: vec![25],
                k: vec![1.0],
                d: vec![2.0],
                j: vec![3.0],
            },
        )]
        .into(),
        series_ids: [("SMA#2".into(), vec![23, 24])].into(),
        last_len: 26,
    };
    let old_json = serde_json::to_value(&old).unwrap();
    let from_json: TaCache = serde_json::from_value(old_json.clone()).unwrap();
    // to_vec deliberately emits a sequence, not the named map from to_vec_named.
    let from_msgpack: TaCache = rmp_serde::from_slice(&rmp_serde::to_vec(&old).unwrap()).unwrap();
    for cache in [from_json, from_msgpack] {
        assert!(cache.signal_references.is_empty());
        assert!(cache.updated_macd_keys.is_none());
        let mut actual = serde_json::to_value(cache).unwrap();
        actual.as_object_mut().unwrap().remove("signal_references");
        assert_eq!(actual, old_json);
    }
}

#[test]
fn new_reference_facts_roundtrip_in_json_and_positional_messagepack() {
    let mut cache = TaCache::default();
    let reference = serde_json::json!({
        "kind": "b1_b3_overlap", "window": {"di": 2, "n": 5},
        "overlap_bis": [], "last_bi": {"position": 5, "direction": "Down",
            "sdt": "2026-01-01T00:00:00Z", "edt": "2026-01-02T00:00:00Z",
            "high": 15.0, "low": 14.0, "finished": true}, "zd": 12.0, "zg": 12.0
    });
    cache
        .signal_references
        .insert("test".into(), serde_json::from_value(reference).unwrap());
    let expected = serde_json::to_value(&cache).unwrap();
    let json: TaCache = serde_json::from_value(expected.clone()).unwrap();
    let msgpack: TaCache = rmp_serde::from_slice(&rmp_serde::to_vec(&cache).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(json).unwrap(), expected);
    assert_eq!(serde_json::to_value(msgpack).unwrap(), expected);
}

#[test]
fn pre_native_structure_positional_reference_prices_remain_unknown() {
    use czsc_core::objects::direction::Direction;
    use czsc_signals::reference::{ReferenceKind, SignalReference};
    let old_bi = (
        5usize,
        Direction::Down,
        "2026-01-01T00:00:00Z",
        "2026-01-02T00:00:00Z",
        15.,
        14.,
        true,
    );
    let old = (
        ReferenceKind::CanonicalZs,
        (2usize, 5usize),
        Vec::<u8>::new(),
        old_bi,
        12.,
        13.,
    );
    let decoded: SignalReference =
        rmp_serde::from_slice(&rmp_serde::to_vec(&old).unwrap()).unwrap();
    assert!(decoded.last_bi.finished);
    assert_eq!((decoded.last_bi.high, decoded.last_bi.low), (15., 14.));
    assert_eq!(decoded.last_bi.start_price, None);
    assert_eq!(decoded.last_bi.end_price, None);
    assert!(decoded.structure.is_none());
}

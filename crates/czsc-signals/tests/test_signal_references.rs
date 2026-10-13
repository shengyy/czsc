use chrono::{TimeZone, Utc};
use czsc_core::{
    analyze::CZSC,
    objects::{
        bar::{NewBarBuilder, RawBar, RawBarBuilder},
        bi::BIBuilder,
        direction::Direction,
        freq::Freq,
        fx::FXBuilder,
        mark::Mark,
    },
};
use czsc_signals::{reference::ReferenceKind, registry::SIGNAL_REGISTRY, types::TaCache};
use serde_json::json;
use std::collections::HashMap;

fn raw(id: i32, close: f64) -> RawBar {
    RawBarBuilder::default()
        .symbol("TEST")
        .id(id)
        .freq(Freq::F30)
        .dt(Utc
            .timestamp_opt(1_700_000_000 + i64::from(id) * 1800, 0)
            .unwrap())
        .open(close)
        .close(close)
        .high(close)
        .low(close)
        .vol(1.0)
        .amount(close)
        .build()
        .unwrap()
}

// Tiny BI geometry fixtures, with native RawBar snapshots for the real MA path.
fn structure(ranges: &[(f64, f64)], first: Direction, rising_ma: bool) -> CZSC {
    let bars: Vec<_> = (0..=ranges.len())
        .map(|i| {
            raw(
                i as i32,
                if rising_ma {
                    10.0 + i as f64
                } else {
                    30.0 - i as f64
                },
            )
        })
        .collect();
    let mut c = CZSC::new(vec![bars[0].clone()], 50, 6);
    c.bars_raw = bars.clone();
    c.bars_ubi.clear();
    c.bi_list = ranges
        .iter()
        .enumerate()
        .map(|(i, &(low, high))| {
            let up = (i % 2 == 0) == (first == Direction::Up);
            let fx = |index: usize, level: f64, mark: Mark| {
                let rb = &bars[index];
                let nb = NewBarBuilder::default()
                    .symbol("TEST")
                    .id(rb.id)
                    .freq(Freq::F30)
                    .dt(rb.dt)
                    .open(rb.open)
                    .close(rb.close)
                    .high(rb.high)
                    .low(rb.low)
                    .vol(1.0)
                    .amount(rb.amount)
                    .elements(vec![rb.clone()])
                    .build()
                    .unwrap();
                FXBuilder::default()
                    .symbol("TEST")
                    .dt(rb.dt)
                    .mark(mark)
                    .fx(level)
                    .high(level)
                    .low(level)
                    .elements(vec![nb])
                    .build()
                    .unwrap()
            };
            BIBuilder::default()
                .symbol("TEST")
                .direction(if up { Direction::Up } else { Direction::Down })
                .fx_a(fx(
                    i,
                    if up { low } else { high },
                    if up { Mark::D } else { Mark::G },
                ))
                .fx_b(fx(
                    i + 1,
                    if up { high } else { low },
                    if up { Mark::G } else { Mark::D },
                ))
                .fxs(vec![])
                .bars(vec![])
                .build()
                .unwrap()
        })
        .collect();
    c
}

fn evaluate(c: &CZSC, name: &str, di: usize, cache: &mut TaCache) -> (String, String) {
    let params: HashMap<_, _> = [
        ("di".to_owned(), json!(di)),
        ("timeperiod".to_owned(), json!(1)),
    ]
    .into();
    let s = (SIGNAL_REGISTRY[name].func)(c, &params, cache);
    (s[0].key(), s[0].value())
}

#[test]
fn third_buy_captures_selected_window_and_updates_same_value() {
    // Every later up BI makes a new high. The first 12-BI window wins.
    let mut ranges = vec![(10.0, 14.0); 12];
    for i in (0..10).step_by(2) {
        ranges[i] = (10.0 + i as f64 / 10.0, 14.0 + i as f64 / 10.0);
    }
    ranges[11] = (15.0, 16.0);
    let mut c = structure(&ranges, Direction::Up, true);
    let mut cache = TaCache::default();
    let (key, value) = evaluate(&c, "cxt_third_buy_V230228", 1, &mut cache);
    assert_eq!(value, "三买_12笔_任意_0");
    let r = cache.signal_references[&key].clone();
    assert_eq!(r.kind, ReferenceKind::KeyUpBiOverlap);
    assert_eq!((r.zd, r.zg), (10.8, 14.0));
    assert_eq!(
        r.overlap_bis.iter().map(|b| b.position).collect::<Vec<_>>(),
        [1, 3, 5, 7, 9]
    );
    assert!(!r.last_bi.finished);
    assert!(r.overlap_bis.iter().all(|b| b.finished));
    assert_eq!(r.window.n, 12);
    c.bi_list[0].fx_b.high = 14.1;
    assert_eq!(
        evaluate(&c, "cxt_third_buy_V230228", 1, &mut cache).1,
        value
    );
    assert_eq!(cache.signal_references[&key].zg, 14.1);
    assert_ne!(cache.signal_references[&key], r);
    c.bi_list[11].fx_b.low = 13.0;
    assert!(
        evaluate(&c, "cxt_third_buy_V230228", 1, &mut cache)
            .1
            .starts_with("其他")
    );
    assert!(cache.signal_references.is_empty());
    c.bi_list[11].fx_b.low = 15.0;
    evaluate(&c, "cxt_third_buy_V230228", 1, &mut cache);
    assert!(!cache.signal_references.is_empty());
    c.bi_list.truncate(4);
    evaluate(&c, "cxt_third_buy_V230228", 1, &mut cache);
    assert!(cache.signal_references.is_empty());
}

#[test]
fn third_bs_point_overlap_is_symmetric_and_finished_is_window_specific() {
    for buy in [true, false] {
        // The chosen 5-BI window starts at index 2: b1 and b3 touch at 12.
        let mut ranges = vec![(10.0, 14.0); 7];
        ranges[2] = (10.0, 12.0);
        ranges[4] = (12.0, 14.0);
        ranges[6] = if buy { (15.0, 16.0) } else { (8.0, 9.0) };
        let mut c = structure(
            &ranges,
            if buy { Direction::Down } else { Direction::Up },
            buy,
        );
        let mut cache = TaCache::default();
        let (key, value) = evaluate(&c, "cxt_third_bs_V230318", 1, &mut cache);
        assert_eq!(
            value,
            if buy {
                "三买_任意_任意_0"
            } else {
                "三卖_任意_任意_0"
            }
        );
        let r = cache.signal_references[&key].clone();
        assert_eq!(r.kind, ReferenceKind::B1B3Overlap);
        assert_eq!((r.zd, r.zg), (12.0, 12.0));
        assert_eq!(
            r.overlap_bis.iter().map(|b| b.position).collect::<Vec<_>>(),
            [1, 3]
        );
        assert!(!r.last_bi.finished);
        let last = c.bi_list.last().unwrap().clone();
        c.bi_list.push(last);
        let (di_key, di_value) = evaluate(&c, "cxt_third_bs_V230318", 2, &mut cache);
        assert_eq!(di_value, value);
        assert!(cache.signal_references[&di_key].last_bi.finished);
        assert_eq!(cache.signal_references[&di_key].window.di, 2);
        // Same label, different overlap operands.
        c.bi_list[2].fx_a.high = 12.5;
        c.bi_list[2].fx_b.high = 12.5;
        assert_eq!(evaluate(&c, "cxt_third_bs_V230318", 2, &mut cache).1, value);
        assert_eq!(cache.signal_references[&di_key].zg, 12.5);
        // Non-overlap, then insufficient structure, both remove only their key.
        c.bi_list[4].fx_a.low = 13.0;
        c.bi_list[4].fx_b.low = 13.0;
        assert!(
            evaluate(&c, "cxt_third_bs_V230318", 2, &mut cache)
                .1
                .starts_with("其他")
        );
        assert!(!cache.signal_references.contains_key(&di_key));
        c.bi_list.truncate(2);
        evaluate(&c, "cxt_third_bs_V230318", 1, &mut cache);
        assert!(cache.signal_references.is_empty());
    }
}

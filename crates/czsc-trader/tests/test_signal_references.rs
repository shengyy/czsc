use chrono::{Duration, TimeZone, Utc};
use czsc_core::objects::{
    bar::{RawBar, RawBarBuilder},
    freq::Freq,
    market::Market,
};
use czsc_trader::{
    czsc_signals::CzscSignals,
    engine_v2::compiler::{ExecutionPlan, ExecutionPlanInput},
    sig_parse::SignalConfig,
};
use czsc_utils::bar_generator::BarGenerator;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

fn bars() -> Vec<RawBar> {
    let start = Utc.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap();
    (0..2500)
        .map(|i| {
            let x = i as f64;
            let close = 100.0 + x * 0.02 + 8.0 * (x * 0.035).sin() + 3.0 * (x * 0.17).sin();
            let slot = i % 8;
            let minute = if slot < 4 {
                600 + slot * 30
            } else {
                810 + (slot - 4) * 30
            };
            RawBarBuilder::default()
                .symbol("TEST")
                .id(i)
                .freq(Freq::F30)
                .dt(start + Duration::days(i64::from(i / 8)) + Duration::minutes(i64::from(minute)))
                .open(close - 0.1)
                .close(close)
                .high(close + 0.3)
                .low(close - 0.3)
                .vol(100.0)
                .amount(close * 100.0)
                .build()
                .unwrap()
        })
        .collect()
}

fn configs() -> Vec<SignalConfig> {
    ["cxt_third_buy_V230228", "cxt_third_bs_V230318"]
        .into_iter()
        .flat_map(|name| {
            [1, 2].map(|di| SignalConfig {
                name: name.to_owned(),
                freq: Some("30分钟".to_owned()),
                params: [("di".to_owned(), json!(di))].into(),
            })
        })
        .collect()
}

fn engine(bars: &[RawBar]) -> CzscSignals {
    let bg = BarGenerator::new(Freq::F30, vec![Freq::D], 5000, Market::AShare).unwrap();
    for bar in bars {
        bg.update_bar(bar).unwrap();
    }
    CzscSignals::new("TEST".to_owned(), bg)
}

fn refs(c: &CzscSignals) -> Value {
    serde_json::to_value(c.signal_references()).unwrap()
}

fn hit(input: &[RawBar], cfg: &[SignalConfig]) -> (CzscSignals, usize) {
    let mut c = engine(&input[..300]);
    for (i, bar) in input.iter().enumerate().skip(300) {
        c.update_signals(bar, cfg).unwrap();
        if c.signal_references().keys().any(|k| k.contains("三买辅助")) {
            return (c, i);
        }
    }
    panic!("fixture must exercise a real third_buy hit");
}

#[test]
fn hot_clone_and_serde_reuse_ema_result_on_duplicate_bar() {
    let mut input = bars();
    input.truncate(100);
    for (i, bar) in input.iter_mut().enumerate() {
        let close = if i < 56 {
            1000.0
        } else if i == 99 {
            100.1
        } else {
            100.0
        };
        bar.open = close;
        bar.close = close;
        bar.high = close + 0.3;
        bar.low = close - 0.3;
    }
    let cfg = vec![SignalConfig {
        name: "tas_ma_base_V221101".into(),
        freq: Some("30分钟".into()),
        params: [
            ("ma_type".into(), json!("EMA")),
            ("timeperiod".into(), json!(34)),
        ]
        .into(),
    }];
    let mut c = engine(&input);
    let last = input.last().unwrap();
    c.update_signals(last, &cfg).unwrap();
    assert_eq!(c.signal_map.len(), 1);
    assert_eq!(c.signal_map.values().next().unwrap(), "空头_向下_任意_0");
    let named: CzscSignals = rmp_serde::from_slice(&rmp_serde::to_vec_named(&c).unwrap()).unwrap();
    let msgpack: CzscSignals = rmp_serde::from_slice(&rmp_serde::to_vec(&c).unwrap()).unwrap();
    for (name, mut restored) in [
        ("clone", c.clone()),
        ("named messagepack", named),
        ("messagepack", msgpack),
    ] {
        for _ in 0..2 {
            restored.update_signals(last, &cfg).unwrap();
            assert_eq!(restored.s, c.s, "{name} must reuse the existing EMA result");
        }
    }
}

#[test]
fn stream_references_clone_serde_and_tail_revisions_follow_current_signals() {
    let input = bars();
    let cfg = configs();
    let mut c = engine(&input[..300]);
    let mut previous: HashMap<String, czsc_signals::reference::SignalReference> = HashMap::new();
    let mut previous_values = HashMap::new();
    let (mut seen, mut changed, mut cleared) = (HashSet::new(), HashSet::new(), HashSet::new());
    for bar in &input[300..] {
        c.update_signals(bar, &cfg).unwrap();
        let current: HashMap<_, _> = c
            .signal_references()
            .into_iter()
            .map(|(k, v)| (k, v.clone()))
            .collect();
        for (key, r) in &current {
            assert!(c.signal_map[key].starts_with("三买") || c.signal_map[key].starts_with("三卖"));
            seen.insert(key.clone());
            if previous
                .get(key)
                .is_some_and(|old: &czsc_signals::reference::SignalReference| {
                    previous_values.get(key) == c.signal_map.get(key) && old != r
                })
            {
                changed.insert(key.clone());
            }
            let structure = &c.kas["30分钟"];
            let end = structure.bi_list.len() + 1 - r.window.di;
            assert_eq!(
                r.last_bi.finished,
                end <= structure.finished_bis_slice().len()
            );
        }
        for key in previous.keys().filter(|k| !current.contains_key(*k)) {
            cleared.insert(key.clone());
        }
        previous = current;
        previous_values = c.signal_map.clone();
    }
    assert_eq!(seen.len(), 4);
    assert_eq!(changed.len(), 2); // di=2 windows stay fixed in this smooth fixture.
    assert_eq!(cleared.len(), 4);

    let (c, i) = hit(&input, &cfg);
    let snapshot = refs(&c);
    let restored: CzscSignals =
        rmp_serde::from_slice(&rmp_serde::to_vec_named(&c).unwrap()).unwrap();
    assert_eq!(refs(&restored), snapshot);
    let mut revised = input[i].clone();
    revised.high *= 1.02;
    revised.low *= 0.98;
    for mut variant in [c.clone(), restored] {
        variant.update_signals(&input[i], &cfg).unwrap();
        assert_eq!(refs(&variant), snapshot);
        // Existing BG contract ignores a duplicate base timestamp, even with
        // changed OHLC. References must follow the retained structure/signal.
        variant.update_signals(&revised, &cfg).unwrap();
        assert_eq!(refs(&variant), snapshot);
        for bar in &input[i + 1..i + 20] {
            variant.update_signals(bar, &cfg).unwrap();
        }
        let mut replay = c.clone();
        for bar in &input[i + 1..i + 20] {
            replay.update_signals(bar, &cfg).unwrap();
        }
        assert_eq!(variant.s, replay.s);
        assert_eq!(refs(&variant), refs(&replay));
    }
}

#[test]
fn configuration_content_changes_invalidate_unchanged_frequency_results() {
    let input = bars();
    let mut cfg = configs();
    let (c, i) = hit(&input, &cfg);
    let mut c: CzscSignals = rmp_serde::from_slice(&rmp_serde::to_vec(&c).unwrap()).unwrap();
    let original = refs(&c);
    c.update_signals(&input[i], &cfg).unwrap();
    assert_eq!(refs(&c), original);
    let ptr = cfg.as_ptr();
    for config in &mut cfg {
        config.params.insert("di".to_owned(), json!(99));
    }
    assert_eq!(ptr, cfg.as_ptr());
    c.update_signals(&input[i], &cfg).unwrap();
    assert!(c.signal_references().is_empty());
    assert!(
        !c.signal_map
            .keys()
            .any(|k| k.contains("_D1_") || k.contains("_D2_"))
    );
    cfg = configs();
    c.update_signals(&input[i], &cfg).unwrap();
    assert_eq!(refs(&c), original);
    cfg.clear();
    c.update_signals(&input[i], &cfg).unwrap();
    assert!(c.signal_references().is_empty());
    assert!(c.signal_map.is_empty());
    c = rmp_serde::from_slice(&rmp_serde::to_vec(&c).unwrap()).unwrap();
    // A disabled frequency misses several bars, then resumes from current BG.
    for b in &input[i + 1..i + 11] {
        c.update_signals(b, &cfg).unwrap();
    }
    cfg = configs();
    c.update_signals(&input[i + 10], &cfg).unwrap();
    let mut fresh = engine(&input[..i + 11]);
    fresh.update_signals(&input[i + 10], &cfg).unwrap();
    assert_eq!(c.s, fresh.s);
    assert_eq!(refs(&c), refs(&fresh));
    for config in &mut cfg {
        config.freq = Some("日线".to_owned());
    }
    c.update_signals(&input[i + 10], &cfg).unwrap();
    assert!(c.signal_references().keys().all(|k| k.starts_with("日线_")));
    assert!(c.signal_map.keys().all(|k| k.starts_with("日线_")));
}

fn plan(cfg: Vec<SignalConfig>) -> ExecutionPlan {
    ExecutionPlan::compile(ExecutionPlanInput {
        symbol: "TEST".to_owned(),
        base_freq: "30分钟".to_owned(),
        signals_config: cfg,
        positions: vec![
            serde_json::from_value(
                json!({"opens": [], "exits": [], "interval": 0, "timeout": 1,
            "stop_loss": 0.0, "T0": false, "name": "inactive", "symbol": "TEST"}),
            )
            .unwrap(),
        ],
        market: Some("A股".to_owned()),
        bg_max_count: Some(5000),
        sdt: None,
        include_sdt_bar: None,
    })
    .unwrap()
}

#[test]
fn compiled_plan_replacement_clears_references_and_reuses_current_operands() {
    let input = bars();
    let cfg = configs();
    let (mut c, i) = hit(&input, &cfg);
    let expected = refs(&c);
    let compiled = plan(cfg.clone());
    c.load_compiled_signal_plan(&compiled.signal_plan).unwrap();
    assert!(c.signal_references().is_empty());
    c.update_signals(&input[i], &[]).unwrap();
    assert_eq!(refs(&c), expected);
    c.load_compiled_signal_plan(&plan(vec![]).signal_plan)
        .unwrap();
    assert!(c.signal_references().is_empty());
    c.update_signals(&input[i], &cfg).unwrap(); // plan owns configuration
    assert!(c.signal_references().is_empty());
    assert!(c.signal_map.is_empty());
}

#[test]
fn aggregated_same_time_tail_refreshes_reference_operands() {
    let mut input = bars();
    let start = input[0].dt - Duration::minutes(600);
    for (i, bar) in input.iter_mut().enumerate() {
        let slot = i % 48;
        let minute = if slot < 24 {
            575 + slot * 5
        } else {
            785 + (slot - 24) * 5
        };
        bar.freq = Freq::F5;
        bar.dt = start + Duration::days((i / 48) as i64) + Duration::minutes(minute as i64);
    }
    let bg = BarGenerator::new(Freq::F5, vec![Freq::F30], 5000, Market::AShare).unwrap();
    for bar in &input[..300] {
        bg.update_bar(bar).unwrap();
    }
    let mut c = CzscSignals::new("TEST".into(), bg);
    let cfg = configs();
    let mut previous = refs(&c);
    let mut previous_values = HashMap::new();
    let mut previous_dt = None;
    let mut changed = HashSet::new();
    for bar in &input[300..] {
        c.update_signals(bar, &cfg).unwrap();
        let structure = &c.kas["30分钟"];
        let dt = structure.bars_raw.last().unwrap().dt;
        let current = refs(&c);
        if previous_dt == Some(dt) {
            for (key, r) in c.signal_references() {
                if previous.get(&key).is_some_and(|old| old != &current[&key])
                    && previous_values.get(&key) == c.signal_map.get(&key)
                {
                    changed.insert(r.kind);
                    let index = structure.bi_list.len() - r.window.di;
                    assert_eq!(r.last_bi.high, structure.bi_list[index].get_high());
                    assert_eq!(r.last_bi.low, structure.bi_list[index].get_low());
                }
            }
        }
        previous = current;
        previous_values = c.signal_map.clone();
        previous_dt = Some(dt);
    }
    assert_eq!(changed.len(), 2);
}

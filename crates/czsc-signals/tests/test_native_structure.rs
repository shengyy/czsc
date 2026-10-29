#[path = "support/bs_fixture.rs"]
mod fixture;
use czsc_signals::{registry::SIGNAL_REGISTRY, types::TaCache};
use fixture::{evaluate, structure};
use serde_json::json;
use std::collections::HashMap;

#[test]
fn actual_six_leg_material_passes_without_experience_or_ma_gate() {
    for buy in [true, false] {
        let prices: Vec<_> = [10., 20., 15., 19., 16., 24., 22.]
            .into_iter()
            .map(|p| if buy { p } else { 40. - p })
            .collect();
        let c = structure(&prices);
        let mut cache = TaCache::default();
        let name = "cxt_third_bs_V230318";
        let params: HashMap<_, _> = [("timeperiod".into(), json!(100_000))].into();
        let s = (SIGNAL_REGISTRY[name].func)(&c, &params, &mut cache);
        assert!(s[0].value().starts_with(if buy { "三买" } else { "三卖" }));
        if buy {
            assert!(
                evaluate(&c, "cxt_third_buy_V230228", 1, &mut cache)
                    .1
                    .starts_with("三买")
            );
        }
    }
}

use czsc_core::{
    analyze::utils::get_zs_seq,
    objects::{bar::NewBar, direction::Direction},
};
use czsc_signals::{
    structure::{StructureSnapshot, get_structure, get_structure_state},
    utils::{
        bi_power::bi_power,
        bs::{PointKind, evaluate_first, evaluate_second, evaluate_second_like, evaluate_third},
        macd::{STRUCTURE_MACD_KEY, native_macd, update_macd_cache},
    },
};
use fixture::{powered_structure, raw};

// Check the complete lightweight contract, not only eligible labels. Failed
// references, core/candidate identity and missing reasons must also agree.
fn checked_structure(c: &czsc_core::analyze::CZSC, di: usize) -> StructureSnapshot {
    let full = get_structure(c, di);
    let full_json = serde_json::to_value(&full).unwrap();
    let light = serde_json::to_value(get_structure_state(c, di)).unwrap();
    assert_eq!(light.as_object().unwrap().len(), 9);
    for key in [
        "basis",
        "di",
        "as_of",
        "status",
        "previous_zs",
        "latest_finished_zs",
        "zs_candidate",
        "candidate_stroke",
        "results",
    ] {
        assert_eq!(light.get(key).unwrap(), &full_json[key], "{key}");
    }
    full
}

#[test]
fn macd_coverage_warmup_nonfinite_and_zero_area_are_not_filled_with_zero() {
    let prices = [20., 10., 15., 11., 14., 9.];
    let short = structure(&prices);
    let result = evaluate_first(&short, 5, Direction::Down, &native_macd(&short));
    assert_eq!(
        (result.status, result.reason),
        ("unavailable", Some("warmup"))
    );
    checked_structure(&short, 1);
    let c = powered_structure(&prices);
    let source = native_macd(&c);
    let eval = evaluate_first(&c, 5, Direction::Down, &source);
    assert!(eval.confirmed());
    let cmp = eval.point.unwrap().macd_comparison.unwrap();
    assert!(cmp.ratio.unwrap() < 1.);
    for i in 0..5 {
        let p = bi_power(&c, &source, &c.bi_list[i]);
        assert_eq!(p.macd.sample_count, 20);
        assert_eq!(p.macd.ready_count, 20);
        let indexes: Vec<_> = c
            .bars_raw
            .iter()
            .enumerate()
            .filter(|(_, b)| b.dt > p.sdt && b.dt <= p.edt)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            p.macd.negative_area,
            Some(indexes.iter().map(|i| (-source.macd[*i]).max(0.)).sum())
        );
        assert_eq!(
            p.macd.hist_at_end,
            Some(source.macd[*indexes.last().unwrap()])
        );
    }
    let mut bad = c.clone();
    bad.bars_raw[70].close = f64::NAN;
    assert_eq!(
        evaluate_first(&bad, 5, Direction::Down, &native_macd(&bad)).reason,
        Some("non_finite")
    );
    let p = bi_power(&bad, &native_macd(&bad), &bad.bi_list[4]);
    assert_eq!(p.macd.negative_area, None);
    let mut missing = c.clone();
    missing.bars_raw.remove(42);
    assert_eq!(
        evaluate_first(&missing, 5, Direction::Down, &native_macd(&missing)).reason,
        Some("missing_coverage")
    );
    checked_structure(&missing, 1);
    let mut zero = c.clone();
    for b in &mut zero.bars_raw {
        b.close = 20.;
    }
    let e = evaluate_first(&zero, 5, Direction::Down, &native_macd(&zero));
    assert_eq!(
        (e.status, e.reason),
        ("unavailable", Some("zero_entry_area"))
    );
    assert_eq!(e.point.unwrap().macd_comparison.unwrap().ratio, None);
    checked_structure(&zero, 1);
    let s = checked_structure(&bad, 1);
    assert!(serde_json::to_string(&s).is_ok());
    assert_eq!(s.macd.hist_scale, 1.);
    assert!(s.macd.bars[..33].iter().all(|b| b.hist.is_none()));
}

#[test]
fn macd_cache_is_identical_for_cold_stream_revision_and_retention() {
    let mut c = powered_structure(&[
        20., 10., 15., 11., 14., 9., 16., 12., 18., 13., 20., 15., 22.,
    ]);
    let mut cache = TaCache::default();
    for n in [200, 230, c.bars_raw.len()] {
        let mut prefix = c.clone();
        prefix.bars_raw.truncate(n);
        update_macd_cache(&prefix, STRUCTURE_MACD_KEY, 12, 26, 9, &mut cache);
    }
    c.bars_raw.last_mut().unwrap().close += 1.;
    update_macd_cache(&c, STRUCTURE_MACD_KEY, 12, 26, 9, &mut cache);
    let same = |c: &_, cache: &TaCache| {
        let expected = native_macd(c);
        let actual = &cache.macd[STRUCTURE_MACD_KEY];
        for (x, y) in expected
            .dif
            .iter()
            .chain(&expected.dea)
            .chain(&expected.macd)
            .zip(actual.dif.iter().chain(&actual.dea).chain(&actual.macd))
        {
            assert!(
                x == y || x.is_nan() && y.is_nan(),
                "cold and warm must be byte-identical"
            );
        }
    };
    same(&c, &cache);
    c.bars_raw.drain(..20);
    update_macd_cache(&c, STRUCTURE_MACD_KEY, 12, 26, 9, &mut cache);
    same(&c, &cache);
}

fn forming(prices: &[f64], tail: &[f64]) -> czsc_core::analyze::CZSC {
    let mut c = powered_structure(prices);
    let anchor = c.bars_raw.last().unwrap().clone();
    c.bars_ubi = vec![NewBar::new_from_raw(&anchor)];
    for price in tail {
        let bar = raw(c.bars_raw.len() as i32, *price);
        c.bars_ubi.push(NewBar::new_from_raw(&bar));
        c.bars_raw.push(bar);
    }
    c
}

#[test]
fn forming_second_and_third_have_native_anchors_but_are_never_confirmed_signals() {
    for buy in [true, false] {
        for second in [true, false] {
            let prices = if second {
                vec![20., 10., 15., 11., 14., 9., 16.]
            } else {
                vec![10., 20., 15., 19., 16., 24.]
            };
            let tail = if second { 15.5 } else { 23.5 };
            let prices: Vec<_> = prices
                .iter()
                .map(|p| if buy { *p } else { 40. - p })
                .collect();
            let c = forming(&prices, &[if buy { tail } else { 40. - tail }]);
            let direction = if buy { Direction::Down } else { Direction::Up };
            let snapshot = checked_structure(&c, 1);
            let kind = if second {
                PointKind::Second
            } else {
                PointKind::Third
            };
            let r = snapshot
                .results
                .iter()
                .find(|e| e.kind == kind && e.direction == direction)
                .unwrap();
            assert_eq!(r.status, "forming_retest", "{r:?}");
            assert!(r.reference.as_ref().unwrap().retest.is_none());
            assert_eq!(snapshot.candidate_stroke.role, "first_retest_candidate");
            for name in [
                if buy {
                    "cxt_first_buy_V221126"
                } else {
                    "cxt_first_sell_V221126"
                },
                "cxt_second_bs_V230320",
                "cxt_third_buy_V230228",
                "cxt_third_bs_V230318",
            ] {
                assert!(
                    evaluate(&c, name, 1, &mut TaCache::default())
                        .1
                        .starts_with("其他")
                );
            }
            // Equality and a wick break cannot be hidden by the NewBar endpoint.
            let mut breached = c.clone();
            let boundary = if second { 9. } else { 19. };
            // Keep the candidate NewBar's real price owner; add a separate
            // raw wick. Replacing its only owner would correctly make the
            // candidate unavailable, which is a different missing-source case.
            let wick = raw(
                breached.bars_raw.len() as i32,
                if buy { boundary } else { 40. - boundary },
            );
            breached.bars_raw.push(wick);
            let e = if second {
                evaluate_second(
                    &breached,
                    breached.bi_list.len(),
                    direction,
                    &native_macd(&breached),
                    true,
                )
            } else {
                evaluate_third(&breached, breached.bi_list.len(), direction, true)
            };
            assert_eq!(e.reason, Some("boundary_touched"));
            // The reverse NewBar remains, but a later raw new extreme invalidates
            // the fixed anchor. A candidate cannot travel along a breakout.
            let mut new_high = c.clone();
            let anchor = prices.last().unwrap();
            if buy {
                new_high.bars_raw.last_mut().unwrap().high = anchor + 0.1;
            } else {
                new_high.bars_raw.last_mut().unwrap().low = anchor - 0.1;
            }
            let e = if second {
                evaluate_second(
                    &new_high,
                    new_high.bi_list.len(),
                    direction,
                    &native_macd(&new_high),
                    true,
                )
            } else {
                evaluate_third(&new_high, new_high.bi_list.len(), direction, true)
            };
            assert_eq!(e.reason, Some("anchor_exceeded"));
        }
    }
}

#[test]
fn pure_breakout_extension_and_withdrawn_old_retest_do_not_revive() {
    let c = forming(&[10., 20., 15., 19., 16., 24.], &[24.5]);
    assert_eq!(c.get_candidate_stroke().kind.as_deref(), Some("extension"));
    assert!(
        !checked_structure(&c, 1)
            .results
            .iter()
            .any(|e| e.status == "forming_retest")
    );
    let mut old = powered_structure(&[10., 20., 15., 19., 16., 24., 22.]);
    assert!(evaluate_third(&old, 6, Direction::Down, false).confirmed());
    old.bars_raw.push(raw(old.bars_raw.len() as i32, 24.1));
    let e = evaluate_third(&old, 6, Direction::Down, false);
    assert_eq!(e.reason, Some("anchor_exceeded"));
    // A new genuine downward BI is evaluated on its own current latest core.
    let fresh = powered_structure(&[10., 20., 15., 19., 16., 24., 22., 25., 23., 26., 25.]);
    assert!(evaluate_third(&fresh, 10, Direction::Down, false).confirmed());
}

#[test]
fn centre_geometry_is_unchanged_and_di_never_claims_to_be_history() {
    let c = forming(&[10., 20., 15., 19., 16., 24., 22., 25., 23., 26.], &[25.5]);
    let snap = checked_structure(&c, 1);
    let valid: Vec<_> = get_zs_seq(c.finished_bis_slice())
        .into_iter()
        .filter(|z| z.bis.len() >= 3 && z.is_valid())
        .collect();
    let z = valid.last().unwrap();
    let r = snap.latest_finished_zs.unwrap();
    assert_eq!(
        (r.sdt, r.edt, r.zd, r.zg, r.dd, r.gg),
        (z.sdt, z.edt, z.zd, z.zg, z.dd, z.gg)
    );
    let sliced = checked_structure(&c, 2);
    assert_eq!(
        sliced.candidate_stroke.reason.as_deref(),
        Some("di_is_not_history")
    );
    assert!(!sliced.results.iter().any(|e| e.status == "forming_retest"));
    for zone in [sliced.previous_zs, sliced.latest_finished_zs]
        .into_iter()
        .flatten()
    {
        assert!(zone.retest.is_none_or(|b| b.position <= sliced.bis.len()));
    }
    assert_eq!(checked_structure(&c, 0).status, "missing_material");
}

#[test]
fn second_like_requires_the_new_core_seed_to_be_the_concrete_b2_retest() {
    let prices = [20., 10., 15., 11., 14., 9., 18., 16., 20., 17., 19., 18.];
    for buy in [true, false] {
        let prices: Vec<_> = prices
            .iter()
            .map(|p| if buy { *p } else { 40. - p })
            .collect();
        let c = powered_structure(&prices);
        let dir = if buy { Direction::Down } else { Direction::Up };
        let e = evaluate_second_like(&c, 11, dir, &native_macd(&c));
        assert!(
            e.confirmed(),
            "{e:?}; zones={:?}; parent={:?}",
            get_zs_seq(c.finished_bis_slice())
                .iter()
                .map(|z| (z.sdt, z.zd, z.zg, z.bis.len()))
                .collect::<Vec<_>>(),
            evaluate_second(&c, 7, dir, &native_macd(&c), false)
        );
        let point = e.point.unwrap();
        assert_eq!(point.parent_chain, Some([4, 5, 6]));
        assert_eq!(point.departure, 9);
        assert!(point.macd_comparison.is_none());
        // A generic higher low with no one-buy parent is not a second-like.
        let mut no_parent = c.clone();
        for b in &mut no_parent.bars_raw {
            b.close = 20.;
        }
        assert!(!evaluate_second_like(&no_parent, 11, dir, &native_macd(&no_parent)).confirmed());
        // Never continue the tag into arbitrary later retests of that core.
        assert!(!evaluate_second_like(&c, 9, dir, &native_macd(&c)).confirmed());
    }
}

#[test]
fn weaker_is_macd_not_price_volume_or_duration_contraction() {
    let c = powered_structure(&[20., 10., 15., 11., 14., 9.]);
    // Equal duration and volume: the former AND (volume OR length) rule fails.
    assert_eq!(c.bi_list[0].get_length(), c.bi_list[4].get_length());
    assert_eq!(
        c.bi_list[0].get_power_volume(),
        c.bi_list[4].get_power_volume()
    );
    assert!(evaluate_first(&c, 5, Direction::Down, &native_macd(&c)).confirmed());
    // Same exact BI geometry, a late entry impulse has little completed area;
    // the outgoing area is stronger. Price-only contraction cannot qualify it.
    let mut stronger = c.clone();
    for b in &mut stronger.bars_raw[40..59] {
        b.open = 20.;
        b.close = 20.;
        b.high = 20.;
        b.low = 20.;
    }
    let result = evaluate_first(&stronger, 5, Direction::Down, &native_macd(&stronger));
    assert_eq!(result.reason, Some("macd_not_weaker"));
    assert!(
        result
            .point
            .unwrap()
            .macd_comparison
            .unwrap()
            .ratio
            .unwrap()
            >= 1.
    );
}

#[test]
fn long_valid_third_point_is_not_vetoed_by_declining_ma() {
    let c = powered_structure(&[
        100., 80., 90., 60., 70., 40., 50., 20., 30., 10., 14., 11., 15., 12., 14., 11., 15., 12.,
        14., 11., 15., 12., 14., 11., 15., 12., 14., 11., 18., 16.,
    ]);
    let e = evaluate_third(&c, c.bi_list.len(), Direction::Down, false);
    assert!(e.confirmed(), "{e:?}");
    let params: HashMap<_, _> = [("timeperiod".into(), json!(1000))].into();
    let mut cache = TaCache::default();
    let s = (SIGNAL_REGISTRY["cxt_third_bs_V230318"].func)(&c, &params, &mut cache);
    assert!(s[0].value().starts_with("三买"));
    assert!(
        cache.series.is_empty(),
        "this owner must not calculate a veto MA"
    );
    assert_eq!(
        cache.signal_references[&s[0].key()]
            .structure
            .as_ref()
            .unwrap()
            .stage,
        "confirmed_valid"
    );
}

#[test]
fn lightweight_prefix_and_same_dt_revision_preserve_exact_state() {
    use czsc_core::analyze::CZSC;
    let bars = powered_structure(&[
        20., 10., 18., 16., 21., 12., 19., 17., 23., 14., 20., 18., 25., 16., 24., 20.,
    ])
    .bars_raw;
    let mut c = CZSC::new(bars[..40].to_vec(), 8, 6);
    let mut available = 0;
    for (i, bar) in bars.iter().enumerate().skip(40) {
        c.update_bar(bar.clone());
        checked_structure(&c, 1);
        let original = get_structure_state(&c, 1);
        let cold = CZSC::new(bars[..=i].to_vec(), 8, 6);
        assert_eq!(original, get_structure_state(&cold, 1));
        available += usize::from(original.candidate_stroke.state == "available");
        if i % 17 == 0 {
            let mut revised = bar.clone();
            revised.high += 3.;
            revised.low -= 2.;
            c.update_bar(revised.clone());
            checked_structure(&c, 1);
            let mut revised_prefix = bars[..i].to_vec();
            revised_prefix.push(revised);
            assert_eq!(
                get_structure_state(&c, 1),
                get_structure_state(&CZSC::new(revised_prefix, 8, 6), 1)
            );
            c.update_bar(bar.clone());
            checked_structure(&c, 1);
            assert_eq!(get_structure_state(&c, 1), original);
            checked_structure(&c, 0);
            checked_structure(&c, 2);
        }
    }
    assert!(available > 20);
    assert!(
        c.bars_raw.len() < bars.len(),
        "exercise actual retained history"
    );
}

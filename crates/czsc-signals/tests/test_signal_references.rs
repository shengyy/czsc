#[path = "support/bs_fixture.rs"]
mod fixture;
use czsc_core::analyze::utils::get_zs_seq;
use czsc_signals::{reference::ReferenceKind, types::TaCache};
use fixture::{evaluate, raw, structure};

const THIRD: &[f64] = &[
    10., 14., 11., 15., 12., 14., 11., 15., 12., 14., 11., 18., 16.,
];

#[test]
fn third_buy_captures_latest_centre_and_updates_same_value() {
    let mut c = structure(THIRD);
    let mut cache = TaCache::default();
    let (key, value) = evaluate(&c, "cxt_third_buy_V230228", 1, &mut cache);
    assert_eq!(value, "三买_11笔_任意_0");
    let r = cache.signal_references[&key].clone();
    assert_eq!(r.kind, ReferenceKind::CanonicalZs);
    assert_eq!((r.zd, r.zg), (12., 14.));
    assert_eq!(
        r.overlap_bis.iter().map(|b| b.position).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert!(!r.last_bi.finished);
    assert!(r.overlap_bis.iter().all(|b| b.finished));
    // Change both ends of the shared peak, preserving contiguous BIs.
    c.bi_list[0].fx_b.high = 14.1;
    c.bi_list[1].fx_a.high = 14.1;
    assert_eq!(
        evaluate(&c, "cxt_third_buy_V230228", 1, &mut cache).1,
        value
    );
    assert_eq!(cache.signal_references[&key].zg, 14.1);
    assert_ne!(cache.signal_references[&key], r);
    // Last BI still passes, but a subsequent raw bar touched the ZG boundary.
    c.bars_raw.push(raw(13, 14.1));
    assert!(
        evaluate(&c, "cxt_third_buy_V230228", 1, &mut cache)
            .1
            .starts_with("其他")
    );
    assert!(cache.signal_references.is_empty());
    c.bars_raw.pop();
    evaluate(&c, "cxt_third_buy_V230228", 1, &mut cache);
    assert!(!cache.signal_references.is_empty());
    c.bi_list.truncate(4);
    evaluate(&c, "cxt_third_buy_V230228", 1, &mut cache);
    assert!(cache.signal_references.is_empty());
}

#[test]
fn third_bs_is_symmetric_and_incomplete_tail_does_not_hide_the_centre() {
    for buy in [true, false] {
        let prices: Vec<_> = THIRD
            .iter()
            .map(|&p| if buy { p } else { 30. - p })
            .collect();
        let mut c = structure(&prices);
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
        assert_eq!(
            cache.signal_references[&key].kind,
            ReferenceKind::CanonicalZs
        );
        let original = cache.signal_references[&key].clone();
        // Completing the pullback adds a singleton to zs_list; the latest
        // valid >=3-BI centre must still be the preceding one.
        c.bars_ubi = c.bi_list[0].bars.clone();
        let zs = get_zs_seq(c.finished_bis_slice());
        assert_eq!(zs.last().unwrap().bis.len(), 1);
        assert_eq!(evaluate(&c, "cxt_third_bs_V230318", 1, &mut cache).1, value);
        assert!(cache.signal_references[&key].last_bi.finished);
        assert_eq!(cache.signal_references[&key].zg, original.zg);
        assert_eq!(cache.signal_references[&key].zd, original.zd);
        // A historical offset is not contaminated by later centres or prices.
        let mut longer = prices.clone();
        longer.extend(if buy {
            [19., 17., 20., 18.]
        } else {
            [11., 13., 10., 12.]
        });
        let historical = structure(&longer);
        let (historical_key, historical_value) =
            evaluate(&historical, "cxt_third_bs_V230318", 5, &mut cache);
        assert_eq!(historical_value, value);
        assert_eq!(cache.signal_references[&historical_key].zg, original.zg);
        assert_eq!(cache.signal_references[&historical_key].zd, original.zd);
        assert!(cache.signal_references[&historical_key].last_bi.finished);
    }
}

#[test]
fn old_overlap_inside_new_centre_and_second_retest_are_rejected() {
    // The first outside retest plus two following BIs establishes [17,18].
    // A later low at 17.5 is inside it although it clears the old [12,14].
    let mut newer = THIRD.to_vec();
    newer.extend([19., 17., 20., 17.5]);
    let c = structure(&newer);
    let zs = get_zs_seq(c.finished_bis_slice());
    let latest = zs
        .iter()
        .rev()
        .find(|z| z.bis.len() >= 3 && z.is_valid())
        .unwrap();
    assert_eq!((latest.zd, latest.zg), (17., 18.));
    for name in ["cxt_third_buy_V230228", "cxt_third_bs_V230318"] {
        assert!(
            evaluate(&c, name, 1, &mut TaCache::default())
                .1
                .starts_with("其他")
        );
    }
    let mut prices = THIRD.to_vec();
    prices.extend([19., 17.]);
    let c = structure(&prices);
    for name in ["cxt_third_buy_V230228", "cxt_third_bs_V230318"] {
        assert!(
            evaluate(&c, name, 1, &mut TaCache::default())
                .1
                .starts_with("其他")
        );
    }
}

#[test]
fn equality_at_centre_boundary_is_not_an_outside_retest() {
    for buy in [true, false] {
        let mut prices = THIRD.to_vec();
        *prices.last_mut().unwrap() = 14.;
        if !buy {
            prices.iter_mut().for_each(|p| *p = 30. - *p);
        }
        let c = structure(&prices);
        assert!(
            evaluate(&c, "cxt_third_bs_V230318", 1, &mut TaCache::default())
                .1
                .starts_with("其他")
        );
    }
}

#[test]
fn an_interior_wick_cannot_be_hidden_by_the_bi_endpoint() {
    use chrono::Duration;
    let mut c = structure(THIRD);
    let mut wick = raw(100, 13.9);
    wick.dt = c.bars_raw[11].dt + Duration::seconds(1);
    c.bars_raw.insert(12, wick);
    assert!(c.bi_list.last().unwrap().get_low() > 14.);
    assert!(
        evaluate(&c, "cxt_third_buy_V230228", 1, &mut TaCache::default())
            .1
            .starts_with("其他")
    );
}

#[test]
fn a_confirmed_rollback_core_replaces_the_old_reference() {
    for buy in [true, false] {
        // The final outside retest confirms [11,14]. Until it is finished,
        // the finished-BI view still contains only the older [15,18] core.
        let prices: Vec<_> = [20., 14., 19., 15., 18., 10., 16., 11., 14.]
            .into_iter()
            .map(|p| if buy { 40. - p } else { p })
            .collect();
        let mut c = structure(&prices);
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
        let old = cache.signal_references[&key].clone();
        assert_eq!((old.zd, old.zg), if buy { (22., 25.) } else { (15., 18.) });
        c.bars_ubi = c.bi_list[0].bars.clone();
        let zones = get_zs_seq(c.finished_bis_slice());
        let latest = zones.last().unwrap();
        assert_eq!(latest.bis.len(), 3);
        assert_eq!(
            (latest.zd, latest.zg),
            if buy { (26., 29.) } else { (11., 14.) }
        );
        assert!(
            evaluate(&c, "cxt_third_bs_V230318", 1, &mut cache)
                .1
                .starts_with("其他")
        );
        assert!(!cache.signal_references.contains_key(&key));
    }
}

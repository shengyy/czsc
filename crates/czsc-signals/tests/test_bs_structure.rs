#[path = "support/bs_fixture.rs"]
mod fixture;
use czsc_core::objects::direction::Direction;
use czsc_signals::{
    types::TaCache,
    utils::bs::{first_point, second_point},
};
use fixture::{evaluate, powered_structure as structure, raw};

#[test]
fn first_points_require_real_divergence_and_are_symmetric() {
    for buy in [true, false] {
        let prices: Vec<_> = [30., 24., 28., 25., 29., 20., 23., 21., 24., 19.]
            .into_iter()
            .map(|p| if buy { p } else { 60. - p })
            .collect();
        let mut c = structure(&prices);
        c.bi_list[4].bars = vec![c.bi_list[4].bars[0].clone(); 9];
        let direction = if buy { Direction::Down } else { Direction::Up };
        let name = if buy {
            "cxt_first_buy_V221126"
        } else {
            "cxt_first_sell_V221126"
        };
        assert_eq!(first_point(&c, 9, direction), Some(5));
        assert!(
            evaluate(&c, name, 1, &mut TaCache::default())
                .1
                .starts_with(if buy { "一买" } else { "一卖" })
        );
        // Volume/length contraction is no longer a veto: the same raw MACD operands still qualify.
        c.bi_list[8].bars = c.bi_list[4].bars.clone();
        assert_eq!(first_point(&c, 9, direction), Some(5));
        c.bi_list[8].bars.truncate(6);
        // A later raw breach invalidates the still-published endpoint.
        c.bars_raw
            .push(raw(c.bars_raw.len() as i32, if buy { 18.9 } else { 41.1 }));
        assert_eq!(first_point(&c, 9, direction), None);
    }
}

#[test]
fn second_point_is_the_first_pullback_and_never_breaks_the_anchor() {
    for buy in [true, false] {
        let prices: Vec<_> = [30., 24., 28., 25., 29., 20., 23., 21., 24., 19., 25., 21.]
            .into_iter()
            .map(|p| if buy { p } else { 60. - p })
            .collect();
        let mut c = structure(&prices);
        c.bi_list[4].bars = vec![c.bi_list[4].bars[0].clone(); 9];
        let direction = if buy { Direction::Down } else { Direction::Up };
        assert!(second_point(&c, 11, direction));
        // Boundary equality is rejected, as is an actual break.
        for p in [19., 18.9] {
            if buy {
                c.bi_list[10].fx_b.low = p;
            } else {
                c.bi_list[10].fx_b.high = 60. - p;
            }
            assert!(!second_point(&c, 11, direction));
        }
        let mut prices = prices.clone();
        prices.extend(if buy { [26., 22.] } else { [34., 38.] });
        let mut later = structure(&prices);
        later.bi_list[4].bars = vec![later.bi_list[4].bars[0].clone(); 9];
        assert!(!second_point(&later, 13, direction));
    }
}

#[test]
fn short_monotonic_five_legs_are_not_a_trend_reversal() {
    let c = structure(&[30., 25., 26., 20., 21., 19.]);
    assert_eq!(first_point(&c, 5, Direction::Down), None);
}

#[test]
fn a_single_centre_with_observable_entry_can_diverge_and_have_a_second_point() {
    for buy in [true, false] {
        let prices: Vec<_> = [20., 10., 15., 11., 14., 9.]
            .into_iter()
            .map(|p| if buy { p } else { 30. - p })
            .collect();
        let direction = if buy { Direction::Down } else { Direction::Up };
        let mut c = structure(&prices);
        c.bi_list[0].bars = vec![c.bi_list[0].bars[0].clone(); 9];
        assert_eq!(first_point(&c, 5, direction), Some(5));
        let mut prices = prices;
        prices.extend(if buy { [16., 12.] } else { [14., 18.] });
        let mut c = structure(&prices);
        c.bi_list[0].bars = vec![c.bi_list[0].bars[0].clone(); 9];
        assert!(second_point(&c, 7, direction));
        // Same centre without an observable entry from outside cannot stand in
        // for a complete entering leg, even if its apparent power is large.
        if buy {
            c.bi_list[0].fx_a.high = 14.;
        } else {
            c.bi_list[0].fx_a.low = 16.;
        }
        assert_eq!(first_point(&c, 5, direction), None);
    }
}

#[test]
fn centre_members_are_not_reinterpreted_as_entries_for_an_opposite_signal() {
    use czsc_core::analyze::utils::get_zs_seq;
    for buy in [true, false] {
        let prices: Vec<_> = [8., 4., 7., 5., 20., 10., 15., 11., 14., 9., 16., 14.5]
            .into_iter()
            .map(|p| if buy { p } else { 30. - p })
            .collect();
        let mut c = structure(&prices);
        c.bi_list[4].bars = vec![c.bi_list[4].bars[0].clone(); 9];
        let direction = if buy { Direction::Down } else { Direction::Up };
        let centres = get_zs_seq(&c.bi_list[..8]);
        let latest = centres.last().unwrap();
        assert!(latest.is_valid() && latest.bis.len() >= 3);
        assert_eq!(latest.bis[0].direction, direction);
        // Here the incoming leg is the prior upward breakout, not the first
        // downward member of its core. All signal families use that same ZS.
        assert_ne!(c.bi_list[3].direction, direction);
        assert_eq!(first_point(&c, 9, direction), None);
        assert!(!second_point(&c, 11, direction));
    }
}

#[test]
fn canonical_core_excludes_the_explicit_entry_in_both_directions() {
    use czsc_core::analyze::utils::get_zs_seq;
    for buy in [true, false] {
        let prices: Vec<_> = [18., 12., 20., 14., 15., 11., 16., 15.5]
            .into_iter()
            .map(|p| if buy { p } else { 40. - p })
            .collect();
        let mut c = structure(&prices);
        c.bi_list[0].bars = vec![c.bi_list[0].bars[0].clone(); 9];
        let direction = if buy { Direction::Down } else { Direction::Up };
        let zones = get_zs_seq(&c.bi_list[..4]);
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].sdt, c.bi_list[1].start_dt());
        assert_eq!(
            (zones[0].zd, zones[0].zg),
            if buy { (14., 15.) } else { (25., 26.) }
        );
        assert_eq!(first_point(&c, 5, direction), Some(5));
        assert!(second_point(&c, 7, direction));
        c.bi_list[0].bars.truncate(6);
        assert_eq!(first_point(&c, 5, direction), Some(5));
    }
}

#[test]
fn reported_entry_boundary_example_uses_the_public_core() {
    use czsc_core::analyze::utils::get_zs_seq;
    let mut c = structure(&[18., 12., 18., 11., 15., 10.]);
    c.bi_list[0].bars = vec![c.bi_list[0].bars[0].clone(); 9];
    let zones = get_zs_seq(&c.bi_list[..4]);
    assert_eq!(
        (zones[0].zd, zones[0].zg, zones[0].dd, zones[0].gg),
        (12., 15., 11., 18.)
    );
    assert_eq!(first_point(&c, 5, Direction::Down), Some(5));
}

#[test]
fn later_departure_closes_the_old_core_instead_of_invalidating_it() {
    use czsc_core::analyze::utils::get_zs_seq;
    use czsc_signals::utils::bs::third_point;
    for buy in [true, false] {
        let prices: Vec<_> = [24., 12., 20., 14., 15., 14.5, 17., 16., 16.5, 11.]
            .into_iter()
            .map(|p| if buy { p } else { 40. - p })
            .collect();
        let c = structure(&prices);
        let zones = get_zs_seq(&c.bi_list[..8]);
        assert_eq!(zones.len(), 2);
        assert_eq!(zones[0].bis.len(), 5);
        assert_eq!(
            (zones[0].zd, zones[0].zg),
            if buy { (14., 15.) } else { (25., 26.) }
        );
        assert!(zones[0].is_valid());
        let before = structure(&prices[..8]);
        let point =
            third_point(&before, 1).expect("first outside retest uses the same public core");
        assert_eq!((point.zd, point.zg), (zones[0].zd, zones[0].zg));
        let direction = if buy { Direction::Down } else { Direction::Up };
        assert_eq!(first_point(&c, 9, direction), None);
    }
}

#[test]
fn second_point_public_signal_uses_only_native_structure_in_both_directions() {
    use czsc_signals::registry::SIGNAL_REGISTRY;
    use serde_json::json;
    use std::collections::HashMap;
    let params: HashMap<_, _> = [
        ("di".to_owned(), json!(1)),
        ("timeperiod".to_owned(), json!(100_000)),
    ]
    .into();
    for buy in [true, false] {
        for (local, entry) in [
            (vec![20., 10., 15., 11., 14., 9., 16., 14.5], 0),
            (vec![18., 12., 20., 14., 15., 11., 16., 15.5], 0),
            (
                vec![30., 24., 28., 25., 29., 20., 23., 21., 24., 19., 30., 25.],
                4,
            ),
        ] {
            let prices: Vec<_> = local
                .iter()
                .map(|&p| if buy { p } else { 60. - p })
                .collect();
            let mut c = structure(&prices);
            c.bi_list[entry].bars = vec![c.bi_list[entry].bars[0].clone(); 9];
            let run = |c: &_| {
                (SIGNAL_REGISTRY["cxt_second_bs_V230320"].func)(c, &params, &mut TaCache::default())
                    [0]
                .value()
            };
            assert_eq!(
                run(&c),
                if buy {
                    "二买_任意_任意_0"
                } else {
                    "二卖_任意_任意_0"
                }
            );
            // A wick breaches the anchor without changing closing prices / SMA.
            let anchor = c.bi_list.len() - 3;
            let boundary = if buy {
                c.bi_list[anchor].get_low()
            } else {
                c.bi_list[anchor].get_high()
            };
            if buy {
                c.bi_list.last_mut().unwrap().fx_b.low = boundary;
                c.bars_raw.last_mut().unwrap().low = boundary;
            } else {
                c.bi_list.last_mut().unwrap().fx_b.high = boundary;
                c.bars_raw.last_mut().unwrap().high = boundary;
            }
            assert!(run(&c).starts_with("其他_"));
        }
    }
}

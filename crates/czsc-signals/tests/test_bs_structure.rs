#[path = "support/bs_fixture.rs"]
mod fixture;
use czsc_core::objects::direction::Direction;
use czsc_signals::{
    types::TaCache,
    utils::bs::{first_point, second_point},
};
use fixture::{evaluate, raw, structure};

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
        // No volume/length decay: geometry and a smaller price leg alone do not qualify.
        c.bi_list[8].bars = c.bi_list[4].bars.clone();
        assert_eq!(first_point(&c, 9, direction), None);
        c.bi_list[8].bars.truncate(6);
        // A later raw breach invalidates the still-published endpoint.
        c.bars_raw.push(raw(10, if buy { 18.9 } else { 41.1 }));
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
            c.bi_list[0].fx_a.high = 15.;
        } else {
            c.bi_list[0].fx_a.low = 15.;
        }
        assert_eq!(first_point(&c, 5, direction), None);
    }
}

#[test]
fn earlier_opposite_centre_does_not_hide_a_member_entry_or_its_second_point() {
    use czsc_core::analyze::utils::get_zs_seq;
    for buy in [true, false] {
        // A prior low centre [5,7] breaks upwards to 20. The next Down BI
        // starts the new centre and is its incoming leg; the previous Up BI
        // is not an incoming Down leg. Mirror the whole path for a first sell.
        let local = [20., 10., 15., 11., 14., 9., 16., 14.5];
        let prices: Vec<_> = [8., 4., 7., 5.]
            .into_iter()
            .chain(local)
            .map(|p| if buy { p } else { 30. - p })
            .collect();
        let direction = if buy { Direction::Down } else { Direction::Up };
        let mut c = structure(&prices);
        c.bi_list[4].bars = vec![c.bi_list[4].bars[0].clone(); 9];
        let centres = get_zs_seq(&c.bi_list[..8]);
        assert_eq!(centres.len(), 2);
        assert!(centres.iter().all(|z| z.bis.len() == 4 && z.is_valid()));
        assert_eq!(centres[1].bis[0].direction, direction);
        assert_ne!(c.bi_list[3].direction, direction);
        // The local structure and its window are invariant to earlier history.
        assert_eq!(first_point(&c, 9, direction), Some(5));
        assert!(second_point(&c, 11, direction));
        c.bi_list[4].bars.truncate(6);
        assert_eq!(first_point(&c, 9, direction), None);
        assert!(!second_point(&c, 11, direction));
    }
}

#[test]
fn second_point_public_signal_combines_real_sma_with_structure_in_both_directions() {
    use czsc_signals::registry::SIGNAL_REGISTRY;
    use serde_json::json;
    use std::collections::HashMap;
    let params: HashMap<_, _> = [
        ("di".to_owned(), json!(1)),
        ("timeperiod".to_owned(), json!(3)),
    ]
    .into();
    for buy in [true, false] {
        for prefix in [&[][..], &[8., 4., 7., 5.][..]] {
            let prices: Vec<_> = prefix
                .iter()
                .copied()
                .chain([20., 10., 15., 11., 14., 9., 16., 14.5])
                .map(|p| if buy { p } else { 30. - p })
                .collect();
            let mut c = structure(&prices);
            let entry = prefix.len();
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
            let boundary = if buy { 9. } else { 21. };
            if buy {
                c.bi_list[entry + 6].fx_b.low = boundary;
                c.bars_raw.last_mut().unwrap().low = boundary;
            } else {
                c.bi_list[entry + 6].fx_b.high = boundary;
                c.bars_raw.last_mut().unwrap().high = boundary;
            }
            assert!(run(&c).starts_with("其他_"));
        }
    }
}

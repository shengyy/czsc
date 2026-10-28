//! Contract for canonical, disjoint BI centre groups. A rolling three-BI
//! overlap is not automatically a new group in this public decomposition.
use chrono::{TimeZone, Utc};
use czsc_core::{
    analyze::utils::get_zs_seq,
    objects::{
        bi::{BI, BIBuilder},
        direction::Direction,
        fx::FXBuilder,
        mark::Mark,
        zs::ZS,
    },
};

fn continuous_bis(prices: &[f64]) -> Vec<BI> {
    let fxs: Vec<_> = prices
        .iter()
        .enumerate()
        .map(|(i, &price)| {
            let is_top = if i == 0 {
                price > prices[1]
            } else {
                price > prices[i - 1]
            };
            FXBuilder::default()
                .symbol("TEST")
                .dt(Utc
                    .timestamp_opt(1_700_000_000 + i as i64 * 1800, 0)
                    .unwrap())
                .mark(if is_top { Mark::G } else { Mark::D })
                .fx(price)
                .high(price)
                .low(price)
                .elements(vec![])
                .build()
                .unwrap()
        })
        .collect();
    fxs.windows(2)
        .map(|pair| {
            BIBuilder::default()
                .symbol("TEST")
                .fx_a(pair[0].clone())
                .fx_b(pair[1].clone())
                .direction(if pair[1].fx > pair[0].fx {
                    Direction::Up
                } else {
                    Direction::Down
                })
                .fxs(vec![])
                .bars(vec![])
                .build()
                .unwrap()
        })
        .collect()
}

#[test]
fn returning_legs_extend_the_old_group_until_the_first_outside_retest() {
    for mirror in [false, true] {
        // Old [15, 18]; then 19->10, 10->16, 16->11, 11->14, 14->9.
        let prices: Vec<_> = [20., 14., 18., 15., 19., 10., 16., 11., 14., 9., 13.]
            .into_iter()
            .map(|p| if mirror { 40. - p } else { p })
            .collect();
        let bis = continuous_bis(&prices);
        let groups = get_zs_seq(&bis[..9]);
        assert_eq!(
            groups.iter().map(|z| z.bis.len()).collect::<Vec<_>>(),
            [7, 2]
        );
        assert!(groups[0].is_valid());
        assert_eq!(
            (groups[0].zd, groups[0].zg),
            if mirror { (22., 25.) } else { (15., 18.) }
        );
        // The local overlap exists, but reusing old members would change the
        // public partition, including its history and all signal references.
        let local = ZS::new(bis[5..8].to_vec());
        assert!(local.is_valid());
        assert_eq!(
            (local.zd, local.zg),
            if mirror { (26., 29.) } else { (11., 14.) }
        );
        let completed = get_zs_seq(&bis);
        assert_eq!(completed[0].bis.len(), groups[0].bis.len());
        assert_eq!(completed[0].edt, groups[0].edt);
        assert_eq!(completed[1].bis.len(), 3);
        assert!(completed[1].is_valid());
    }
}

#[test]
fn boundary_touch_still_returns_to_the_old_group() {
    for mirror in [false, true] {
        for high in [15., 14.9] {
            let prices: Vec<_> = [20., 14., 18., 15., 19., 10., high]
                .into_iter()
                .map(|p| if mirror { 40. - p } else { p })
                .collect();
            let groups = get_zs_seq(&continuous_bis(&prices));
            assert_eq!(groups.len(), if high == 15. { 1 } else { 2 });
            assert!(groups[0].is_valid());
        }
    }
}

#[test]
fn continuous_alternating_prefixes_preserve_valid_centres() {
    fn visit(prices: &mut Vec<f64>, checked: &mut usize) {
        if prices.len() == 8 {
            *checked += 1;
            let bis = continuous_bis(prices);
            for end in 3..=bis.len() {
                let before = get_zs_seq(&bis[..end - 1]);
                let after = get_zs_seq(&bis[..end]);
                assert_eq!(after.iter().map(|z| z.bis.len()).sum::<usize>(), end);
                assert!(
                    after.iter().filter(|z| z.bis.len() >= 3).all(ZS::is_valid),
                    "{prices:?}"
                );
                for (old, new) in before.iter().zip(&after) {
                    if old.bis.len() >= 3 {
                        assert_eq!((new.zd, new.zg), (old.zd, old.zg));
                        assert!(new.is_valid(), "{prices:?}");
                    }
                }
            }
            return;
        }
        for level in 1..=4 {
            let price = level as f64;
            if let Some(&last) = prices.last() {
                if price == last {
                    continue;
                }
                if prices.len() >= 2 && ((price > last) == (last > prices[prices.len() - 2])) {
                    continue;
                }
            }
            prices.push(price);
            visit(prices, checked);
            prices.pop();
        }
    }
    let mut checked = 0;
    visit(&mut vec![], &mut checked);
    assert!(checked > 1000);
}

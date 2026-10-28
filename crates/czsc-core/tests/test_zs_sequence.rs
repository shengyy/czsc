//! Explicit incoming BI, frozen three-BI seed, and confirmed boundary rollback.
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

fn prices(values: &[f64], mirror: bool) -> Vec<f64> {
    values
        .iter()
        .map(|p| if mirror { 60. - p } else { *p })
        .collect()
}

#[test]
fn outside_retest_reassigns_two_tail_bis_without_reusing_members() {
    for mirror in [false, true] {
        let bis = continuous_bis(&prices(
            &[20., 14., 18., 15., 19., 10., 16., 11., 14., 9.],
            mirror,
        ));
        let before = get_zs_seq(&bis[..7]);
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].bis.len(), 6);
        let after = get_zs_seq(&bis[..8]);
        assert_eq!(
            after.iter().map(|z| z.bis.len()).collect::<Vec<_>>(),
            [4, 3]
        );
        assert_eq!(after[0].sdt, bis[1].start_dt());
        assert_eq!(after[0].edt, bis[4].end_dt());
        assert_eq!(after[1].sdt, bis[5].start_dt());
        assert_eq!(after[1].edt, bis[7].end_dt());
        assert_eq!((after[0].zd, after[0].zg), (before[0].zd, before[0].zg));
        assert_eq!(
            (after[1].zd, after[1].zg),
            if mirror { (46., 49.) } else { (11., 14.) }
        );
        assert!(after.iter().all(|z| z.is_valid()));
        assert_eq!(
            after[0].bis.last().unwrap().fx_b.dt,
            after[1].bis[0].fx_a.dt
        );
        let extended = get_zs_seq(&bis);
        assert_eq!(extended[0].edt, after[0].edt);
        assert_eq!(extended[1].bis.len(), 4);
        assert_eq!((extended[1].zd, extended[1].zg), (after[1].zd, after[1].zg));
    }
}

#[test]
fn entering_bi_cannot_inflate_the_core_or_swallow_a_later_centre() {
    for mirror in [false, true] {
        let bis = continuous_bis(&prices(
            &[24., 12., 20., 14., 15., 14.5, 17., 16., 16.5, 11.],
            mirror,
        ));
        let seed = get_zs_seq(&bis[..4]);
        assert_eq!(seed[0].sdt, bis[1].start_dt());
        assert_eq!(
            (seed[0].zd, seed[0].zg),
            if mirror { (45., 46.) } else { (14., 15.) }
        );
        let after = get_zs_seq(&bis);
        assert_eq!(
            after.iter().map(|z| z.bis.len()).collect::<Vec<_>>(),
            [5, 3]
        );
        assert_eq!((after[0].zd, after[0].zg), (seed[0].zd, seed[0].zg));
        assert_eq!(after[0].edt, bis[5].end_dt());
        assert!(after[0].is_valid());
        assert_eq!(after[1].sdt, bis[6].start_dt());
        assert_eq!(
            (after[1].zd, after[1].zg),
            if mirror { (43.5, 44.) } else { (16., 16.5) }
        );
        assert!(after[1].is_valid());
    }
}

#[test]
fn same_interval_extensions_keep_the_initial_three_bi_core() {
    for mirror in [false, true] {
        let bis = continuous_bis(&prices(
            &[24., 12., 20., 14., 18., 15., 19., 14.5, 18., 13.],
            mirror,
        ));
        let groups = get_zs_seq(&bis);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].bis.len(), 8);
        assert_eq!(
            (groups[0].zd, groups[0].zg),
            if mirror { (42., 46.) } else { (14., 18.) }
        );
        assert!(groups[0].is_valid());
    }
}

#[test]
fn boundary_touch_does_not_split_and_short_pending_core_keeps_history() {
    for mirror in [false, true] {
        for high in [15., 14.9] {
            let bis = continuous_bis(&prices(&[20., 14., 18., 15., 19., 10., high, 11.], mirror));
            let groups = get_zs_seq(&bis);
            assert_eq!(groups.len(), if high == 15. { 1 } else { 2 });
            assert_eq!(
                (groups[0].zd, groups[0].zg),
                if mirror { (42., 45.) } else { (15., 18.) }
            );
            assert!(groups[0].is_valid());
            if high == 15. {
                assert_eq!(groups[0].bis.len(), 6);
            } else {
                assert_eq!(groups[0].bis.len(), 4);
                assert_eq!(groups[1].bis.len(), 2);
                assert_eq!(groups[1].sdt, bis[5].start_dt());
            }
        }
    }
}

#[test]
fn outside_retest_starts_new_core_and_the_true_departure_is_its_entry() {
    for mirror in [false, true] {
        let bis = continuous_bis(&prices(
            &[30., 20., 24., 21., 24., 10., 15., 11., 14., 9.],
            mirror,
        ));
        let groups = get_zs_seq(&bis);
        assert_eq!(
            groups.iter().map(|z| z.bis.len()).collect::<Vec<_>>(),
            [4, 4]
        );
        assert_eq!(groups[0].sdt, bis[1].start_dt());
        assert_eq!(groups[1].sdt, bis[5].start_dt());
        assert_eq!(
            (groups[1].zd, groups[1].zg),
            if mirror { (46., 49.) } else { (11., 14.) }
        );
        // The four-member old group cannot surrender its initial seed.
        assert_eq!(groups[0].bis[0].fx_a.dt, bis[1].fx_a.dt);
        assert_eq!(groups[0].bis[2].fx_b.dt, bis[3].fx_b.dt);
        assert_eq!(groups[1].bis[0].fx_a.dt, bis[4].fx_b.dt);
    }
}

#[test]
fn valid_replacement_seed_cannot_steal_the_old_cores_third_member() {
    for mirror in [false, true] {
        let bis = continuous_bis(&prices(&[5., 20., 10., 25., 15., 30., 22.], mirror));
        let replacement = ZS::new(bis[3..6].to_vec());
        assert!(replacement.is_valid());
        assert_eq!(
            (replacement.zd, replacement.zg),
            if mirror { (35., 38.) } else { (22., 25.) }
        );
        let groups = get_zs_seq(&bis);
        assert_eq!(
            groups.iter().map(|z| z.bis.len()).collect::<Vec<_>>(),
            [4, 1]
        );
        assert_eq!(
            (groups[0].zd, groups[0].zg),
            if mirror { (40., 45.) } else { (15., 20.) }
        );
        // Moving the last two old members would remove this initial third BI.
        assert_eq!(groups[0].bis[2].fx_a.dt, bis[3].fx_a.dt);
        assert_eq!(groups[0].bis[2].fx_b.dt, bis[3].fx_b.dt);
        assert_eq!(groups[1].sdt, bis[5].start_dt());
    }
}

#[test]
fn dropping_initial_context_can_change_the_bootstrap_core() {
    for mirror in [false, true] {
        let bis = continuous_bis(&prices(&[30., 20., 25., 21., 24., 22., 23.], mirror));
        let full = get_zs_seq(&bis);
        let trimmed = get_zs_seq(&bis[1..]);
        assert_eq!(full.len(), 1);
        assert_eq!(trimmed.len(), 1);
        assert_eq!(
            (full[0].zd, full[0].zg),
            if mirror { (36., 39.) } else { (21., 24.) }
        );
        assert_eq!(
            (trimmed[0].zd, trimmed[0].zg),
            if mirror { (36., 38.) } else { (22., 24.) }
        );
        assert_eq!(full[0].sdt, bis[1].start_dt());
        assert_eq!(trimmed[0].sdt, bis[2].start_dt());
        // The first retained BI is explicit entry context. This decomposition
        // does not promise invariant centres after deleting that context.
        assert_ne!((full[0].zd, full[0].zg), (trimmed[0].zd, trimmed[0].zg));
    }
}

#[test]
fn unformed_seed_advances_by_two_and_bootstrap_entry_is_not_exported() {
    assert!(get_zs_seq(&[]).is_empty());
    for mirror in [false, true] {
        let bis = continuous_bis(&prices(&[30., 20., 25., 15., 18., 12., 16.], mirror));
        assert!(get_zs_seq(&bis[..1]).is_empty());
        let partial = get_zs_seq(&bis[..3]);
        assert_eq!(partial.len(), 1);
        assert_eq!(partial[0].bis.len(), 2);
        let groups = get_zs_seq(&bis);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].sdt, bis[3].start_dt());
        assert_eq!(groups[0].bis.len(), 3);
        assert_eq!(
            (groups[0].zd, groups[0].zg),
            if mirror { (44., 45.) } else { (15., 16.) }
        );
        assert!(groups[0].is_valid());
    }
}

#[test]
fn alternating_prefixes_keep_formed_cores_valid_and_members_unique() {
    fn visit(prices: &mut Vec<f64>, checked: &mut usize) {
        if prices.len() == 8 {
            *checked += 1;
            let bis = continuous_bis(prices);
            for end in 3..=bis.len() {
                let before = get_zs_seq(&bis[..end - 1]);
                let after = get_zs_seq(&bis[..end]);
                let mut members = std::collections::HashSet::new();
                for z in &after {
                    assert!(z.is_valid(), "{prices:?}");
                    for bi in &z.bis {
                        assert!(members.insert(bi.start_dt()), "reused BI: {prices:?}");
                    }
                }
                assert!(!members.contains(&bis[0].start_dt()));
                for old in before.iter().filter(|z| z.bis.len() >= 3) {
                    let new = after.iter().find(|z| z.sdt == old.sdt).unwrap();
                    assert!(new.bis.len() >= 3);
                    assert_eq!((new.zd, new.zg), (old.zd, old.zg));
                    assert!(new.is_valid(), "{prices:?}");
                }
            }
            return;
        }
        for level in 1..=4 {
            let price = level as f64;
            if let Some(&last) = prices.last()
                && (price == last
                    || (prices.len() >= 2 && ((price > last) == (last > prices[prices.len() - 2]))))
            {
                continue;
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

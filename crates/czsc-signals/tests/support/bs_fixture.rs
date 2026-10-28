use chrono::{TimeZone, Utc};
use czsc_core::{
    analyze::CZSC,
    objects::{
        bar::{NewBar, NewBarBuilder, RawBar, RawBarBuilder},
        bi::BIBuilder,
        direction::Direction,
        freq::Freq,
        fx::FXBuilder,
        mark::Mark,
    },
};
use czsc_signals::{registry::SIGNAL_REGISTRY, types::TaCache};
use serde_json::json;
use std::collections::HashMap;

pub fn raw(id: i32, price: f64) -> RawBar {
    RawBarBuilder::default()
        .symbol("TEST")
        .id(id)
        .freq(Freq::F30)
        .dt(Utc
            .timestamp_opt(1_700_000_000 + i64::from(id) * 1800, 0)
            .unwrap())
        .open(price)
        .close(price)
        .high(price)
        .low(price)
        .vol(1.0)
        .amount(price)
        .build()
        .unwrap()
}

fn nb(bar: &RawBar) -> NewBar {
    NewBarBuilder::default()
        .symbol("TEST")
        .id(bar.id)
        .freq(Freq::F30)
        .dt(bar.dt)
        .open(bar.open)
        .close(bar.close)
        .high(bar.high)
        .low(bar.low)
        .vol(bar.vol)
        .amount(bar.amount)
        .elements(vec![bar.clone()])
        .build()
        .unwrap()
}

/// Explicit continuous BI endpoints. Raw snapshots exercise the real SMA path;
/// leg duration/volume can be varied independently for divergence tests.
pub fn structure(prices: &[f64]) -> CZSC {
    let bars: Vec<_> = prices
        .iter()
        .enumerate()
        .map(|(i, &p)| raw(i as i32, p))
        .collect();
    let mut c = CZSC::new(vec![bars[0].clone()], 50, 6);
    c.bars_raw = bars.clone();
    c.bars_ubi.clear();
    c.bi_list = prices
        .windows(2)
        .enumerate()
        .map(|(i, p)| {
            let up = p[1] > p[0];
            let fx = |index: usize, mark| {
                FXBuilder::default()
                    .symbol("TEST")
                    .dt(bars[index].dt)
                    .mark(mark)
                    .fx(prices[index])
                    .high(prices[index])
                    .low(prices[index])
                    // Two snapshots allow the second-point MA branch to use [-2].
                    .elements(vec![nb(&bars[index]), nb(&bars[index])])
                    .build()
                    .unwrap()
            };
            BIBuilder::default()
                .symbol("TEST")
                .direction(if up { Direction::Up } else { Direction::Down })
                .fx_a(fx(i, if up { Mark::D } else { Mark::G }))
                .fx_b(fx(i + 1, if up { Mark::G } else { Mark::D }))
                .fxs(vec![])
                .bars(vec![nb(&bars[i]); 6])
                .build()
                .unwrap()
        })
        .collect();
    c
}

pub fn evaluate(c: &CZSC, name: &str, di: usize, cache: &mut TaCache) -> (String, String) {
    let params: HashMap<_, _> = [
        ("di".to_owned(), json!(di)),
        ("timeperiod".to_owned(), json!(1)),
    ]
    .into();
    let s = (SIGNAL_REGISTRY[name].func)(c, &params, cache);
    (s[0].key(), s[0].value())
}

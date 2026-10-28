pub use czsc_core::analyze::utils::get_zs_seq;
use czsc_core::analyze::{CZSC, UBI};
use czsc_core::objects::bar::RawBar;
use czsc_core::objects::direction::Direction;
use czsc_core::objects::fx::FX;
use czsc_core::objects::mark::Mark;

#[inline]
pub fn raw_bar_upper(bar: &RawBar) -> f64 {
    bar.high - bar.open.max(bar.close)
}

#[inline]
pub fn raw_bar_lower(bar: &RawBar) -> f64 {
    bar.open.min(bar.close) - bar.low
}

pub fn fx_raw_bars(fx: &FX) -> Vec<RawBar> {
    fx.elements
        .iter()
        .flat_map(|nb| nb.elements.iter().cloned())
        .collect()
}

pub fn fx_power_str(fx: &FX) -> &'static str {
    if fx.elements.len() != 3 {
        return "弱";
    }
    let k1 = &fx.elements[0];
    let k2 = &fx.elements[1];
    let k3 = &fx.elements[2];
    match fx.mark {
        Mark::D => {
            if k3.close > k1.high {
                "强"
            } else if k3.close > k2.high {
                "中"
            } else {
                "弱"
            }
        }
        Mark::G => {
            if k3.close < k1.low {
                "强"
            } else if k3.close < k2.low {
                "中"
            } else {
                "弱"
            }
        }
    }
}

pub fn fx_has_zs(fx: &FX) -> bool {
    if fx.elements.len() != 3 {
        return false;
    }
    let zd = fx
        .elements
        .iter()
        .map(|x| x.low)
        .fold(f64::NEG_INFINITY, f64::max);
    let zg = fx
        .elements
        .iter()
        .map(|x| x.high)
        .fold(f64::INFINITY, f64::min);
    zg >= zd
}

pub fn unique_prices_from_bars(bars: &[RawBar]) -> Vec<f64> {
    let mut prices: Vec<f64> = Vec::with_capacity(bars.len() * 4);
    for b in bars {
        if b.close.is_finite() {
            prices.push(b.close);
        }
        if b.high.is_finite() {
            prices.push(b.high);
        }
        if b.low.is_finite() {
            prices.push(b.low);
        }
        if b.open.is_finite() {
            prices.push(b.open);
        }
    }
    prices.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    prices.dedup_by(|a, b| (*a - *b).abs() <= f64::EPSILON);
    prices
}

pub fn ubi_raw_bars(c: &CZSC) -> Vec<RawBar> {
    c.bars_ubi
        .iter()
        .flat_map(|nb| nb.elements.iter().cloned())
        .collect()
}

pub fn calc_bi_status_values(czsc: &CZSC, ubi_fxs: &[FX]) -> (&'static str, &'static str) {
    let last_bi = czsc.bi_list.last().unwrap();
    let v1 = match last_bi.direction {
        Direction::Down => {
            if czsc.bars_ubi.len() > 7 {
                "向上"
            } else {
                "向下"
            }
        }
        Direction::Up => {
            if czsc.bars_ubi.len() > 7 {
                "向下"
            } else {
                "向上"
            }
        }
    };

    let last_fx = ubi_fxs.last().unwrap();
    let v2 = match last_fx.mark {
        Mark::D => {
            if v1 == "向下" {
                "底分"
            } else {
                "延伸"
            }
        }
        Mark::G => {
            if v1 == "向上" {
                "顶分"
            } else {
                "延伸"
            }
        }
    };
    (v1, v2)
}

pub fn rebuild_ubi(c: &CZSC) -> Option<UBI> {
    if c.bars_ubi.is_empty() || c.bi_list.is_empty() {
        return None;
    }
    let ubi_fxs = c.get_ubi_fxs()?;
    if ubi_fxs.is_empty() {
        return None;
    }
    let raw_bars: Vec<RawBar> = c
        .bars_ubi
        .iter()
        .flat_map(|x| x.elements.iter().cloned())
        .collect();
    let high_bar = raw_bars
        .iter()
        .max_by(|a, b| {
            a.high
                .partial_cmp(&b.high)
                .unwrap_or(std::cmp::Ordering::Less)
        })?
        .clone();
    let low_bar = raw_bars
        .iter()
        .min_by(|a, b| {
            a.low
                .partial_cmp(&b.low)
                .unwrap_or(std::cmp::Ordering::Greater)
        })?
        .clone();
    let direction = if c.bi_list.last().unwrap().direction == Direction::Down {
        Direction::Up
    } else {
        Direction::Down
    };
    Some(UBI {
        symbol: c.symbol.clone(),
        direction,
        high: high_bar.high,
        low: low_bar.low,
        high_bar,
        low_bar,
        bars: c.bars_ubi.clone(),
        raw_bars,
        fxs: ubi_fxs.clone(),
        fx_a: ubi_fxs.first().unwrap().clone(),
    })
}

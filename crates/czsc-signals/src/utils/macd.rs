//! Single native MACD numerical owner. TA-Lib seed/NaN convention, hist = DIF - DEA.
use crate::types::TaCache;
use czsc_core::{analyze::CZSC, objects::state::MacdSeries};

/// 计算 MACD {dif, dea, macd}，对齐 TA-Lib 原始三元组语义：
/// - `dif` = MACD 线
/// - `dea` = 信号线
/// - `macd` = 柱状图
pub fn calc_macd(series: &[f64], short: usize, long: usize, m: usize) -> MacdSeries {
    let len = series.len();
    let mut dif = vec![f64::NAN; len];
    let mut dea = vec![f64::NAN; len];
    let mut macd = vec![f64::NAN; len];
    if len == 0 || short == 0 || long == 0 || m == 0 || len < long {
        return MacdSeries {
            ids: Vec::new(),
            dif,
            dea,
            macd,
        };
    }

    let fast_offset = long.saturating_sub(short);
    let ema_short_tail = calc_ema_talib_style(&series[fast_offset..], short);
    let ema_long = calc_ema_talib_style(series, long);
    let mut dif_input = vec![f64::NAN; len];
    let mut dif_raw = vec![f64::NAN; len];
    for i in 0..len {
        let ema_short_i = if i >= fast_offset {
            ema_short_tail[i - fast_offset]
        } else {
            f64::NAN
        };
        if ema_short_i.is_finite() && ema_long[i].is_finite() {
            dif_raw[i] = ema_short_i - ema_long[i];
            dif_input[i] = dif_raw[i];
        }
    }

    dea = calc_ema_talib_skip_leading_nan(&dif_input, m);

    for i in 0..len {
        if dif_raw[i].is_finite() && dea[i].is_finite() {
            dif[i] = dif_raw[i];
            macd[i] = dif_raw[i] - dea[i];
        }
    }

    MacdSeries {
        ids: Vec::new(),
        dif,
        dea,
        macd,
    }
}

pub(super) fn calc_ema_talib_style(series: &[f64], n: usize) -> Vec<f64> {
    let len = series.len();
    let mut out = vec![f64::NAN; len];
    if len < n || n == 0 {
        return out;
    }
    let alpha = 2.0 / (n as f64 + 1.0);
    let seed = series[..n].iter().sum::<f64>() / n as f64;
    out[n - 1] = seed;
    for i in n..len {
        out[i] = alpha * series[i] + (1.0 - alpha) * out[i - 1];
    }
    out
}

fn calc_ema_talib_skip_leading_nan(series: &[f64], n: usize) -> Vec<f64> {
    let len = series.len();
    let mut out = vec![f64::NAN; len];
    let Some(start) = series.iter().position(|x| x.is_finite()) else {
        return out;
    };
    let tail = calc_ema_talib_style(&series[start..], n);
    for (i, value) in tail.into_iter().enumerate() {
        out[start + i] = value;
    }
    out
}

/// 计算用于信号缓存的 MACD，默认与 `calc_macd` 使用同一 TA-Lib 原始三元组契约。
pub fn calc_macd_cache_style(series: &[f64], short: usize, long: usize, m: usize) -> MacdSeries {
    calc_macd(series, short, long, m)
}

/// 与 `calc_macd` 完全同义，保留仅用于兼容迁移中的旧调用点。
pub fn calc_macd_py_style(series: &[f64], short: usize, long: usize, m: usize) -> MacdSeries {
    calc_macd(series, short, long, m)
}

/// Exact current retained-input calculation. Never reseed a shorter tail window:
/// that approximation made identical snapshots depend on cache history.
/// The per-round key set avoids recomputing for multiple signals in one engine round.
pub fn update_macd_cache(
    c: &CZSC,
    key: &str,
    short: usize,
    long: usize,
    m: usize,
    cache: &mut TaCache,
) {
    if let Some(updated) = &mut cache.updated_macd_keys
        && !updated.insert(key.to_string())
    {
        return;
    }
    let closes: Vec<_> = c.bars_raw.iter().map(|b| b.close).collect();
    let mut series = calc_macd(&closes, short, long, m);
    series.ids = c.bars_raw.iter().map(|b| b.id).collect();
    cache.macd.insert(key.to_string(), series);
    cache.last_len = c.bars_raw.len();
}

pub use update_macd_cache as update_macd_cache_py_style;

pub const STRUCTURE_MACD_KEY: &str = "MACD12#26#9";
pub const HIST_SCALE: f64 = 1.0;
pub const LOOKBACK: usize = 33;

pub fn native_macd(c: &CZSC) -> MacdSeries {
    let mut series = calc_macd(
        &c.bars_raw.iter().map(|b| b.close).collect::<Vec<_>>(),
        12,
        26,
        9,
    );
    series.ids = c.bars_raw.iter().map(|b| b.id).collect();
    series
}

//! MACD and raw interval facts for the same BI operands used by signals.
use super::macd::{HIST_SCALE, LOOKBACK};
use chrono::{DateTime, Utc};
use czsc_core::{
    analyze::CZSC,
    objects::{bi::BI, state::MacdSeries},
};
use serde::{Deserialize, Serialize};

pub fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiMacd {
    pub positive_area: Option<f64>,
    pub negative_area: Option<f64>,
    pub dif_at_start: Option<f64>,
    pub dea_at_start: Option<f64>,
    pub hist_at_start: Option<f64>,
    pub dif_at_end: Option<f64>,
    pub dea_at_end: Option<f64>,
    pub hist_at_end: Option<f64>,
    pub sample_count: usize,
    pub ready_count: usize,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntervalPower {
    pub sdt: DateTime<Utc>,
    pub edt: DateTime<Utc>,
    pub interval: String,
    pub sample_count: usize,
    pub volume: Option<f64>,
    pub amount: Option<f64>,
    pub mean_volume: Option<f64>,
    pub mean_amount: Option<f64>,
    pub duration_seconds: i64,
    pub macd: BiMacd,
}

pub fn interval_power(
    c: &CZSC,
    s: &MacdSeries,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> IntervalPower {
    let rows: Vec<_> = c
        .bars_raw
        .iter()
        .enumerate()
        .filter(|(_, b)| b.dt > start && b.dt <= end)
        .collect();
    let at = |dt| c.bars_raw.iter().position(|b| b.dt == dt);
    let value =
        |v: &[f64], idx: Option<usize>| idx.and_then(|i| v.get(i)).copied().and_then(finite);
    let ready = |i: usize| {
        s.dif.get(i).is_some_and(|x| x.is_finite())
            && s.dea.get(i).is_some_and(|x| x.is_finite())
            && s.macd.get(i).is_some_and(|x| x.is_finite())
    };
    let ready_count = rows.iter().filter(|(i, _)| ready(*i)).count();
    let coverage = start < end
        && at(start).is_some()
        && at(end).is_some()
        && s.ids.len() == c.bars_raw.len()
        && c.bars_raw.iter().enumerate().all(|(i, b)| s.ids[i] == b.id)
        && c.bars_raw.windows(2).all(|w| w[0].dt < w[1].dt);
    // A non-finite close anywhere in the EMA dependency prefix contaminates
    // the interval, even when the affected bar lies before the BI start.
    let non_finite = c
        .bars_raw
        .iter()
        .filter(|b| b.dt <= end)
        .any(|b| !b.close.is_finite());
    let reason = if !coverage || rows.is_empty() {
        Some("missing_coverage")
    } else if non_finite {
        Some("non_finite")
    } else if ready_count != rows.len() {
        Some(if rows.iter().any(|(i, _)| *i < LOOKBACK) {
            "warmup"
        } else {
            "non_finite"
        })
    } else {
        None
    };
    let area = |positive: bool| {
        finite(
            rows.iter()
                .map(|(i, _)| {
                    if positive {
                        s.macd[*i].max(0.)
                    } else {
                        (-s.macd[*i]).max(0.)
                    }
                })
                .sum(),
        )
    };
    let mut macd = BiMacd {
        positive_area: if reason.is_none() { area(true) } else { None },
        negative_area: if reason.is_none() { area(false) } else { None },
        dif_at_start: value(&s.dif, at(start)),
        dea_at_start: value(&s.dea, at(start)),
        hist_at_start: value(&s.macd, at(start)),
        dif_at_end: value(&s.dif, at(end)),
        dea_at_end: value(&s.dea, at(end)),
        hist_at_end: value(&s.macd, at(end)),
        sample_count: rows.len(),
        ready_count,
        status: if reason.is_none() {
            "ready"
        } else {
            "unavailable"
        }
        .into(),
        reason: reason.map(str::to_owned),
    };
    if reason.is_none() && (macd.positive_area.is_none() || macd.negative_area.is_none()) {
        macd.status = "unavailable".into();
        macd.reason = Some("non_finite".into());
        macd.positive_area = None;
        macd.negative_area = None;
    }
    let sum = |amount: bool| {
        if coverage && !rows.is_empty() {
            finite(
                rows.iter()
                    .map(|(_, b)| if amount { b.amount } else { b.vol })
                    .sum(),
            )
        } else {
            None
        }
    };
    let volume = sum(false);
    let amount = sum(true);
    IntervalPower {
        sdt: start,
        edt: end,
        interval: "(sdt,edt]".into(),
        sample_count: rows.len(),
        volume,
        amount,
        mean_volume: volume.map(|v| v / rows.len() as f64),
        mean_amount: amount.map(|v| v / rows.len() as f64),
        duration_seconds: (end - start).num_seconds(),
        macd,
    }
}

pub fn bi_power(c: &CZSC, s: &MacdSeries, bi: &BI) -> IntervalPower {
    let mut power = interval_power(c, s, bi.fx_a.dt, bi.fx_b.dt);
    // Retained BI snapshots let us detect known holes, without inventing a
    // trading calendar or filling missing market bars.
    let missing = bi
        .bars
        .iter()
        .flat_map(|b| &b.elements)
        .filter(|b| b.dt > bi.fx_a.dt && b.dt <= bi.fx_b.dt)
        .any(|b| c.bars_raw.binary_search_by_key(&b.dt, |r| r.dt).is_err());
    if missing {
        power.macd.status = "unavailable".into();
        power.macd.reason = Some("missing_coverage".into());
        power.macd.positive_area = None;
        power.macd.negative_area = None;
        power.volume = None;
        power.amount = None;
        power.mean_volume = None;
        power.mean_amount = None;
    }
    power
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacdBar {
    pub dt: DateTime<Utc>,
    pub dif: Option<f64>,
    pub dea: Option<f64>,
    pub hist: Option<f64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacdCoverage {
    pub sdt: Option<DateTime<Utc>>,
    pub edt: Option<DateTime<Utc>>,
    pub sample_count: usize,
    pub ready_count: usize,
    pub first_ready_at: Option<DateTime<Utc>>,
    pub input: String,
}
/// One maximal run of same-sign histogram bars over the retained raw bars.
/// A zero bar continues the open run (it has no sign of its own) and never
/// starts one; a non-finite bar (warm-up or contaminated input) closes it.
/// No area: force is compared on BI intervals, this view is only the rhythm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacdSegment {
    pub sign: i8,
    pub sdt: DateTime<Utc>,
    pub edt: DateTime<Utc>,
    pub bars: usize,
    /// Signed histogram value with the largest magnitude in the run.
    pub peak: f64,
    pub peak_at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacdFacts {
    pub params: [usize; 3],
    pub hist_scale: f64,
    pub warmup_bars: usize,
    pub bars: Vec<MacdBar>,
    pub segments: Vec<MacdSegment>,
    pub coverage: MacdCoverage,
    pub truncated: bool,
}

pub fn macd_segments(bars: &[MacdBar]) -> Vec<MacdSegment> {
    let mut out: Vec<MacdSegment> = Vec::new();
    let mut open = false;
    for b in bars {
        let Some(h) = b.hist else {
            open = false;
            continue;
        };
        let sign: i8 = if h > 0. {
            1
        } else if h < 0. {
            -1
        } else {
            0
        };
        let joins = open && out.last().is_some_and(|s| sign == 0 || sign == s.sign);
        if joins {
            let s = out.last_mut().unwrap();
            s.edt = b.dt;
            s.bars += 1;
            if h.abs() > s.peak.abs() {
                s.peak = h;
                s.peak_at = b.dt;
            }
        } else if sign != 0 {
            out.push(MacdSegment {
                sign,
                sdt: b.dt,
                edt: b.dt,
                bars: 1,
                peak: h,
                peak_at: b.dt,
            });
            open = true;
        }
    }
    out
}
impl MacdFacts {
    pub fn capture(c: &CZSC, s: &MacdSeries) -> Self {
        let bars: Vec<_> = c
            .bars_raw
            .iter()
            .enumerate()
            .map(|(i, b)| MacdBar {
                dt: b.dt,
                dif: s.dif.get(i).copied().and_then(finite),
                dea: s.dea.get(i).copied().and_then(finite),
                hist: s.macd.get(i).copied().and_then(finite),
            })
            .collect();
        let ready: Vec<_> = bars
            .iter()
            .filter(|b| b.dif.is_some() && b.dea.is_some() && b.hist.is_some())
            .collect();
        let coverage = MacdCoverage {
            sdt: bars.first().map(|b| b.dt),
            edt: bars.last().map(|b| b.dt),
            sample_count: bars.len(),
            ready_count: ready.len(),
            first_ready_at: ready.first().map(|b| b.dt),
            input: "current_retained_bars_raw".into(),
        };
        Self {
            params: [12, 26, 9],
            hist_scale: HIST_SCALE,
            warmup_bars: LOOKBACK,
            segments: macd_segments(&bars),
            bars,
            coverage,
            truncated: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn bar(minute: i64, hist: Option<f64>) -> MacdBar {
        MacdBar {
            dt: Utc.timestamp_opt(minute * 60, 0).unwrap(),
            dif: hist,
            dea: hist.map(|_| 0.),
            hist,
        }
    }

    #[test]
    fn zero_continues_a_run_and_missing_values_close_it() {
        let hist = [
            None,
            Some(0.),
            Some(0.2),
            Some(0.),
            Some(0.5),
            Some(-0.1),
            Some(-0.4),
            None,
            Some(-0.3),
            Some(0.),
        ];
        let bars: Vec<_> = hist
            .iter()
            .enumerate()
            .map(|(i, h)| bar(i as i64, *h))
            .collect();
        let s = macd_segments(&bars);
        let rows: Vec<_> = s
            .iter()
            .map(|x| (x.sign, x.sdt, x.edt, x.bars, x.peak, x.peak_at))
            .collect();
        assert_eq!(
            rows,
            vec![
                (1, bars[2].dt, bars[4].dt, 3, 0.5, bars[4].dt),
                (-1, bars[5].dt, bars[6].dt, 2, -0.4, bars[6].dt),
                (-1, bars[8].dt, bars[9].dt, 2, -0.3, bars[8].dt),
            ]
        );
    }
}

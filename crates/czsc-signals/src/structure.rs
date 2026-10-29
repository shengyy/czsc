//! Public, detached snapshot of the signal owner's native operands and stages.
use crate::{
    reference::{CentreReference, ReferenceBi, StructureReference},
    utils::{
        bi_power::{IntervalPower, MacdFacts, bi_power, interval_power},
        bs::{self, Evaluation, PointKind},
        macd::native_macd,
    },
};
use chrono::{DateTime, Utc};
use czsc_core::{
    analyze::{CZSC, candidate::CandidateStroke, utils::get_zs_seq},
    objects::{direction::Direction, state::MacdSeries},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrokeFact {
    #[serde(flatten)]
    pub bi: ReferenceBi,
    pub included_in_finished_bis: bool,
    pub price_amplitude: f64,
    pub newbar_count: usize,
    pub power: IntervalPower,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureResult {
    pub kind: PointKind,
    pub direction: Direction,
    pub status: String,
    pub reason: Option<String>,
    pub reference: Option<StructureReference>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ZsRelation {
    pub basis: String,
    pub core: String,
    pub outer: String,
}
/// Lightweight, detached stage/operand view for per-bar prefix observation.
/// References retain qualification operands, including needed MACD comparisons;
/// no all-BI power projection or per-bar numerical series is constructed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureState {
    pub basis: String,
    pub di: usize,
    pub as_of: Option<DateTime<Utc>>,
    pub status: String,
    pub previous_zs: Option<CentreReference>,
    pub latest_finished_zs: Option<CentreReference>,
    pub zs_candidate: Option<CentreReference>,
    pub candidate_stroke: CandidateStroke,
    pub results: Vec<StructureResult>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureSnapshot {
    pub basis: String,
    pub di: usize,
    pub as_of: Option<DateTime<Utc>>,
    pub status: String,
    pub bis: Vec<StrokeFact>,
    pub last_signal_bi: Option<StrokeFact>,
    pub ubi_newbar_count: usize,
    pub previous_zs: Option<CentreReference>,
    pub latest_finished_zs: Option<CentreReference>,
    pub zs_candidate: Option<CentreReference>,
    pub zs_relation: Option<ZsRelation>,
    pub candidate_stroke: CandidateStroke,
    pub candidate_power: Option<IntervalPower>,
    pub macd: MacdFacts,
    pub results: Vec<StructureResult>,
}
fn relation(al: f64, ah: f64, bl: f64, bh: f64) -> String {
    (if bl > ah {
        "above"
    } else if bh < al {
        "below"
    } else if bl == ah || bh == al {
        "touch"
    } else {
        "overlap"
    })
    .into()
}
/// `di` slices today's list; it never reconstructs a historical state. Replay
/// actual input prefixes for first-known times. Returns all retained MACD bars;
/// no output truncation, no hidden resampling, and no recursive validity claim.
pub fn get_structure(c: &CZSC, di: usize) -> StructureSnapshot {
    let macd = native_macd(c);
    let state = evaluate_structure(c, di, &macd);
    let n = bs::prefix_end(c, di).unwrap_or(0);
    let bis: Vec<_> = (0..n)
        .map(|i| {
            let b = &c.bi_list[i];
            StrokeFact {
                bi: ReferenceBi::at(c, i),
                included_in_finished_bis: i < c.finished_bis_slice().len(),
                price_amplitude: b.get_power_price(),
                newbar_count: b.bars.len(),
                power: bi_power(c, &macd, b),
            }
        })
        .collect();
    let zs_relation = state
        .previous_zs
        .as_ref()
        .zip(state.latest_finished_zs.as_ref())
        .map(|(a, b)| ZsRelation {
            basis: "bi_proxy".into(),
            core: relation(a.zd, a.zg, b.zd, b.zg),
            outer: relation(a.dd, a.gg, b.dd, b.gg),
        });
    let candidate_power = if state.candidate_stroke.state == "available" {
        state
            .candidate_stroke
            .anchor_at
            .zip(state.candidate_stroke.as_of)
            .map(|(s, e)| interval_power(c, &macd, s, e))
    } else {
        None
    };
    StructureSnapshot {
        basis: state.basis,
        di: state.di,
        as_of: state.as_of,
        status: state.status,
        last_signal_bi: bis.last().cloned(),
        bis,
        ubi_newbar_count: c.bars_ubi.len(),
        previous_zs: state.previous_zs,
        latest_finished_zs: state.latest_finished_zs,
        zs_candidate: state.zs_candidate,
        zs_relation,
        candidate_stroke: state.candidate_stroke,
        candidate_power,
        macd: MacdFacts::capture(c, &macd),
        results: state.results,
    }
}

/// Read current stages, references and candidate/core identities without the
/// full numerical projection. Shares evaluation and qualification with
/// `get_structure`; `di` is still a current-list slice, never a historical state.
/// The caller observes first-known times by replaying actual input prefixes.
pub fn get_structure_state(c: &CZSC, di: usize) -> StructureState {
    evaluate_structure(c, di, &native_macd(c))
}

// The sole stage/identity projection for both public views. Full-only numerical
// fields live in get_structure, so the light path cannot construct then filter
// them. MACD is still required for first-point/parent qualification.
fn evaluate_structure(c: &CZSC, di: usize, macd: &MacdSeries) -> StructureState {
    let end = bs::prefix_end(c, di);
    let n = end.unwrap_or(0);
    let finished = c.finished_bis_slice().len().min(n);
    let zones = get_zs_seq(&c.bi_list[..finished]);
    let mut valid = Vec::new();
    let mut zs_candidate = None;
    for z in zones {
        let start = c
            .bi_list
            .iter()
            .position(|b| b.fx_a.dt == z.bis[0].fx_a.dt && b.fx_b.dt == z.bis[0].fx_b.dt)
            .unwrap();
        let r = CentreReference::capture(c, start, start + z.bis.len(), n);
        if r.valid {
            valid.push(r);
        } else if z.bis.len() <= 2 {
            zs_candidate = Some(r);
        }
    }
    let latest_finished_zs = valid.pop();
    let previous_zs = valid.pop();
    let mut candidate = c.get_candidate_stroke();
    if di != 1 {
        candidate.state = "unavailable".into();
        candidate.reason = Some("di_is_not_history".into());
        candidate.kind = None;
        candidate.direction = None;
    }
    let mut evals: Vec<Evaluation> = Vec::new();
    if let Some(end) = end {
        for dir in [Direction::Down, Direction::Up] {
            evals.push(bs::evaluate_first(c, end, dir, macd));
            for kind in [PointKind::Second, PointKind::Third] {
                let eval = |forming| {
                    if kind == PointKind::Second {
                        bs::evaluate_second(c, end, dir, macd, forming)
                    } else {
                        bs::evaluate_third(c, end, dir, forming)
                    }
                };
                let confirmed = eval(false);
                let forming = (di == 1).then(|| eval(true));
                evals.push(
                    if confirmed.confirmed() || c.bi_list[end - 1].direction == dir {
                        confirmed
                    } else {
                        forming.unwrap_or(confirmed)
                    },
                );
            }
            evals.push(bs::evaluate_second_like(c, end, dir, macd));
        }
    }
    if evals.iter().any(|e| e.status == "forming_retest") {
        candidate.role = "first_retest_candidate".into();
    }
    let results = evals
        .iter()
        .map(|e| StructureResult {
            kind: e.kind,
            direction: e.direction,
            status: e.status.into(),
            reason: e.reason.map(str::to_owned),
            reference: e
                .point
                .as_ref()
                .map(|p| StructureReference::capture(c, p, e.status)),
        })
        .collect();
    StructureState {
        basis: "bi_proxy".into(),
        di,
        as_of: c.bars_raw.last().map(|b| b.dt),
        status: if end.is_some() {
            "ready"
        } else {
            "missing_material"
        }
        .into(),
        previous_zs,
        latest_finished_zs,
        zs_candidate,
        candidate_stroke: candidate,
        results,
    }
}

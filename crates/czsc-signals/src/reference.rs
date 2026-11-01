//! Operands captured in the signal hit, including canonical ZS boundaries.

use chrono::{DateTime, Utc};
use czsc_core::{analyze::CZSC, objects::bi::BI, objects::direction::Direction};
use serde::{Deserialize, Serialize};

/// The overlap used by the signal's price comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    // Retained for decoding snapshots written before the canonical correction.
    KeyUpBiOverlap,
    B1B3Overlap,
    CanonicalZs,
}

/// A participating BI. `position` is one-based within the hit window.
/// Times are BI endpoints, never signal confirmation times.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceBi {
    pub position: usize,
    pub direction: Direction,
    pub sdt: DateTime<Utc>,
    pub edt: DateTime<Utc>,
    pub high: f64,
    pub low: f64,
    /// Membership in the current structure's finished BI prefix.
    pub finished: bool,
    /// Appended optional fields preserve old JSON and positional MessagePack.
    #[serde(default)]
    pub start_price: Option<f64>,
    #[serde(default)]
    pub end_price: Option<f64>,
}

/// The selected window ending at the `di`-th BI from the end of `bi_list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceWindow {
    pub di: usize,
    pub n: usize,
}

/// Current hit facts, indexed by the unchanged Signal key in CzscSignals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalReference {
    pub kind: ReferenceKind,
    pub window: ReferenceWindow,
    /// Only the BIs actually used for the overlap, in window order.
    pub overlap_bis: Vec<ReferenceBi>,
    /// The window's final BI used in the hit's price comparison.
    pub last_bi: ReferenceBi,
    pub zd: f64,
    pub zg: f64,
    #[serde(default)]
    pub structure: Option<StructureReference>,
}

impl SignalReference {
    /// Snapshot already-selected operands; does not select BIs or evaluate a signal.
    pub(crate) fn capture(
        c: &CZSC,
        di: usize,
        bis: &[BI],
        overlap_positions: &[usize],
        kind: ReferenceKind,
        bounds: (f64, f64),
    ) -> Self {
        let start = c.bi_list.len() + 1 - di - bis.len();
        let finished_len = c.finished_bis_slice().len();
        let snapshot = |position: usize| {
            let bi = &bis[position - 1];
            ReferenceBi {
                position,
                direction: bi.direction,
                sdt: bi.fx_a.dt,
                edt: bi.fx_b.dt,
                high: bi.get_high(),
                low: bi.get_low(),
                start_price: Some(bi.fx_a.fx),
                end_price: Some(bi.fx_b.fx),
                finished: start + position <= finished_len,
            }
        };
        Self {
            kind,
            window: ReferenceWindow { di, n: bis.len() },
            overlap_bis: overlap_positions.iter().map(|&p| snapshot(p)).collect(),
            last_bi: snapshot(bis.len()),
            zd: bounds.0,
            zg: bounds.1,
            structure: None,
        }
    }
}

use crate::utils::bs::{Boundary, Evaluation, MacdComparison, Point, PointKind};
use czsc_core::{analyze::candidate::CandidateStroke, objects::zs::ZsView};

impl ReferenceBi {
    /// Global one-based current-list position for role-bearing references.
    pub fn at(c: &CZSC, index: usize) -> Self {
        let b = &c.bi_list[index];
        Self {
            position: index + 1,
            direction: b.direction,
            sdt: b.fx_a.dt,
            edt: b.fx_b.dt,
            high: b.get_high(),
            low: b.get_low(),
            start_price: Some(b.fx_a.fx),
            end_price: Some(b.fx_b.fx),
            finished: index < c.finished_bis_slice().len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CentreReference {
    pub sdt: DateTime<Utc>,
    pub edt: DateTime<Utc>,
    pub zd: f64,
    pub zg: f64,
    pub zz: f64,
    pub dd: f64,
    pub gg: f64,
    pub bi_count: usize,
    pub valid: bool,
    pub members: Vec<ReferenceBi>,
    /// The BI before the first member. Departure/retest are qualified roles of
    /// a structure result, never guessed from the core's own last member.
    pub entry: Option<ReferenceBi>,
}
impl CentreReference {
    pub fn capture(c: &CZSC, start: usize, end: usize) -> Self {
        let z = ZsView::new(&c.bi_list[start..end]);
        Self {
            sdt: z.sdt,
            edt: z.edt,
            zd: z.zd,
            zg: z.zg,
            zz: z.zz,
            dd: z.dd,
            gg: z.gg,
            bi_count: z.bis.len(),
            valid: z.bis.len() >= 3 && z.is_valid(),
            members: (start..end).map(|i| ReferenceBi::at(c, i)).collect(),
            entry: start.checked_sub(1).map(|i| ReferenceBi::at(c, i)),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct B2Chain {
    pub first_point_anchor: ReferenceBi,
    pub rebound: ReferenceBi,
    pub first_retest: Option<ReferenceBi>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureReference {
    pub basis: String,
    pub kind: PointKind,
    pub stage: String,
    pub direction: Direction,
    pub centre: CentreReference,
    pub entry: ReferenceBi,
    pub departure: ReferenceBi,
    pub rebound: Option<ReferenceBi>,
    pub retest: Option<ReferenceBi>,
    pub first_point_anchor: Option<ReferenceBi>,
    pub b2_chain: Option<B2Chain>,
    pub candidate_stroke: Option<CandidateStroke>,
    pub macd_comparison: Option<MacdComparison>,
    /// The stage's binding qualifying boundary, as guarded by its evaluator.
    /// Appended for decoding snapshots written before it existed.
    #[serde(default)]
    pub boundary: Option<Boundary>,
}
impl StructureReference {
    pub fn capture(c: &CZSC, p: &Point, stage: &str) -> Self {
        let r = |i| ReferenceBi::at(c, i);
        let chain = p
            .parent_chain
            .map(|[a, b, d]| B2Chain {
                first_point_anchor: r(a),
                rebound: r(b),
                first_retest: Some(r(d)),
            })
            .or_else(|| {
                if p.kind == PointKind::Second {
                    Some(B2Chain {
                        first_point_anchor: r(p.first_point_anchor?),
                        rebound: r(p.rebound?),
                        first_retest: p.retest.map(r),
                    })
                } else {
                    None
                }
            });
        let centre = CentreReference::capture(c, p.centre_start, p.centre_end);
        let candidate_stroke = p.candidate.then(|| {
            let mut s = c.get_candidate_stroke();
            if stage == "forming_retest" {
                s.role = "first_retest_candidate".into();
            }
            s
        });
        Self {
            basis: "bi_proxy".into(),
            kind: p.kind,
            stage: stage.into(),
            direction: p.direction,
            centre,
            entry: r(p.entry),
            departure: r(p.departure),
            rebound: p.rebound.map(r),
            retest: p.retest.map(r),
            first_point_anchor: p.first_point_anchor.map(r),
            b2_chain: chain,
            candidate_stroke,
            macd_comparison: p.macd_comparison.clone(),
            boundary: Some(p.boundary),
        }
    }
}
impl SignalReference {
    pub fn from_evaluation(c: &CZSC, di: usize, e: &Evaluation) -> Option<Self> {
        if !e.confirmed() {
            return None;
        }
        let p = e.point.as_ref()?;
        let positions: Vec<_> = (p.centre_start..p.centre_start + 3)
            .map(|i| i - p.start + 1)
            .collect();
        let mut r = Self::capture(
            c,
            di,
            &c.bi_list[p.start..p.end],
            &positions,
            ReferenceKind::CanonicalZs,
            (p.zd, p.zg),
        );
        r.structure = Some(StructureReference::capture(c, p, e.status));
        Some(r)
    }
}

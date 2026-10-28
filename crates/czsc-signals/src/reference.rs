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
        }
    }
}

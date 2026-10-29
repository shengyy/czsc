//! The single BI-proxy structure evaluator for public CXT values and references.
//! No recursive trend claim, persistent events, or inferred observation times.
use super::{
    bi_power::{BiMacd, bi_power},
    macd::native_macd,
};
use czsc_core::{
    analyze::{CZSC, utils::get_zs_seq},
    objects::{bi::BI, direction::Direction, state::MacdSeries, zs::ZS},
};
use serde::{Deserialize, Serialize};

pub fn prefix_end(c: &CZSC, di: usize) -> Option<usize> {
    (di > 0 && di <= c.bi_list.len()).then(|| c.bi_list.len() + 1 - di)
}

pub fn valid_centres(bis: &[BI]) -> Vec<(usize, ZS)> {
    get_zs_seq(bis)
        .into_iter()
        .filter_map(|zs| {
            if zs.bis.len() < 3
                || !zs.is_valid()
                || ![zs.zd, zs.zg, zs.dd, zs.gg].iter().all(|v| v.is_finite())
            {
                return None;
            }
            let first = zs.bis.first()?;
            let start = bis
                .iter()
                .position(|bi| bi.fx_a.dt == first.fx_a.dt && bi.fx_b.dt == first.fx_b.dt)?;
            Some((start, zs))
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointKind {
    First,
    Second,
    Third,
    SecondLike,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacdComparison {
    pub direction: Direction,
    pub entry: BiMacd,
    pub departure: BiMacd,
    pub entry_area: Option<f64>,
    pub departure_area: Option<f64>,
    pub ratio: Option<f64>,
    pub status: String,
    pub reason: Option<String>,
}

/// Indices refer exclusively to this current list (end exclusive).
#[derive(Debug, Clone)]
pub struct Point {
    pub kind: PointKind,
    pub direction: Direction,
    pub start: usize,
    pub end: usize,
    pub centre_start: usize,
    pub centre_end: usize,
    pub zd: f64,
    pub zg: f64,
    pub entry: usize,
    pub departure: usize,
    pub rebound: Option<usize>,
    pub retest: Option<usize>,
    pub first_point_anchor: Option<usize>,
    pub parent_chain: Option<[usize; 3]>,
    pub candidate: bool,
    pub macd_comparison: Option<MacdComparison>,
}
#[derive(Debug, Clone)]
pub struct Evaluation {
    pub kind: PointKind,
    pub direction: Direction,
    pub status: &'static str,
    pub reason: Option<&'static str>,
    pub point: Option<Point>,
}
impl Evaluation {
    fn miss(kind: PointKind, direction: Direction, reason: &'static str) -> Self {
        Self {
            kind,
            direction,
            status: "no_match",
            reason: None,
            point: None,
        }
        .reject(reason)
    }
    fn reject(mut self, reason: &'static str) -> Self {
        self.status = if matches!(
            reason,
            "warmup" | "non_finite" | "missing_coverage" | "zero_entry_area"
        ) {
            "unavailable"
        } else {
            "no_match"
        };
        self.reason = Some(reason);
        self
    }
    pub fn confirmed(&self) -> bool {
        self.status == "confirmed_valid"
    }
    pub fn eligible(&self) -> bool {
        self.confirmed() || self.status == "forming_retest"
    }
}

fn finite_bi(b: &BI) -> bool {
    [b.get_high(), b.get_low(), b.fx_a.fx, b.fx_b.fx]
        .iter()
        .all(|v| v.is_finite())
        && b.fx_a.dt < b.fx_b.dt
}
fn adjacent(a: &BI, b: &BI) -> bool {
    finite_bi(a)
        && finite_bi(b)
        && a.direction != b.direction
        && a.fx_b.dt == b.fx_a.dt
        && a.fx_b.fx == b.fx_a.fx
}

/// Complete retained raw path. For di>1, cutoff is the selected endpoint, not
/// an as-of historical analyzer. New highs/lows after a retest anchor reject a
/// withdrawn old BI instead of reviving it as today's first retest.
fn guard_path(
    c: &CZSC,
    end: usize,
    begin: usize,
    direction: Direction,
    boundary: f64,
    cap: Option<f64>,
    inclusive: bool,
) -> Result<(), &'static str> {
    let start = c.bi_list[begin].fx_b.dt;
    let cutoff = if end == c.bi_list.len() {
        c.bars_raw.last().map(|b| b.dt)
    } else {
        Some(c.bi_list[end - 1].fx_b.dt)
    }
    .ok_or("missing_coverage")?;
    if !boundary.is_finite() || cap.is_some_and(|v| !v.is_finite()) {
        return Err("non_finite");
    }
    if !c.bars_raw.iter().any(|b| b.dt == start)
        || !c.bars_raw.iter().any(|b| b.dt == cutoff)
        || cutoff < start
        || !c.bars_raw.windows(2).all(|w| w[0].dt < w[1].dt)
    {
        return Err("missing_coverage");
    }
    if c.bi_list[begin..end]
        .iter()
        .flat_map(|bi| &bi.bars)
        .chain(c.bars_ubi.iter())
        .flat_map(|b| &b.elements)
        .filter(|b| b.dt > start && b.dt <= cutoff)
        .any(|b| c.bars_raw.binary_search_by_key(&b.dt, |r| r.dt).is_err())
    {
        return Err("missing_coverage");
    }
    for b in c.bars_raw.iter().filter(|b| b.dt > start && b.dt <= cutoff) {
        if !b.low.is_finite() || !b.high.is_finite() {
            return Err("non_finite");
        }
        let price = if direction == Direction::Down {
            b.low
        } else {
            b.high
        };
        let holds = if direction == Direction::Down {
            if inclusive {
                price >= boundary
            } else {
                price > boundary
            }
        } else if inclusive {
            price <= boundary
        } else {
            price < boundary
        };
        if !holds {
            return Err("boundary_touched");
        }
        if cap.is_some_and(|cap| {
            if direction == Direction::Down {
                b.high > cap
            } else {
                b.low < cap
            }
        }) {
            return Err("anchor_exceeded");
        }
    }
    Ok(())
}

pub fn evaluate_first(c: &CZSC, end: usize, direction: Direction, macd: &MacdSeries) -> Evaluation {
    let miss = |r| Evaluation::miss(PointKind::First, direction, r);
    let Some(last) = end.checked_sub(1).and_then(|i| c.bi_list.get(i)) else {
        return miss("missing_material");
    };
    if last.direction != direction {
        return miss("direction");
    }
    let centres = valid_centres(&c.bi_list[..end - 1]);
    let Some((start, zs)) = centres.last() else {
        return miss("no_valid_centre");
    };
    let Some(entry_index) = start.checked_sub(1) else {
        return miss("missing_entry");
    };
    let entry = &c.bi_list[entry_index];
    if *start + zs.bis.len() != end - 1 || entry.direction != direction {
        return miss("not_same_centre_departure");
    }
    if !c.bi_list[entry_index..end].iter().all(finite_bi) {
        return miss("non_finite");
    }
    if !c.bi_list[entry_index..end]
        .windows(2)
        .all(|w| adjacent(&w[0], &w[1]))
    {
        return miss("invalid_operands");
    }
    let geometry = if direction == Direction::Down {
        entry.get_high() > zs.zg
            && entry.get_low() <= zs.zg
            && last.get_high() >= zs.zd
            && last.get_low() < zs.dd.min(entry.get_low())
    } else {
        entry.get_low() < zs.zd
            && entry.get_high() >= zs.zd
            && last.get_low() <= zs.zg
            && last.get_high() > zs.gg.max(entry.get_high())
    };
    if !geometry {
        return miss("no_new_extreme");
    }
    let entry_macd = bi_power(c, macd, entry).macd;
    let departure_macd = bi_power(c, macd, last).macd;
    let area = |b: &BiMacd| {
        if direction == Direction::Down {
            b.negative_area
        } else {
            b.positive_area
        }
    };
    let a = area(&entry_macd);
    let b = area(&departure_macd);
    let reason = match (a, b) {
        (Some(0.), _) => Some("zero_entry_area"),
        (Some(a), Some(b)) if a.is_finite() && b.is_finite() && (b / a).is_finite() => None,
        _ => Some(
            if entry_macd.reason.as_deref() == Some("missing_coverage")
                || departure_macd.reason.as_deref() == Some("missing_coverage")
            {
                "missing_coverage"
            } else if entry_macd.reason.as_deref() == Some("non_finite")
                || departure_macd.reason.as_deref() == Some("non_finite")
            {
                "non_finite"
            } else {
                "warmup"
            },
        ),
    };
    let ratio = if reason.is_none() {
        Some(b.unwrap() / a.unwrap())
    } else {
        None
    };
    let comparison = MacdComparison {
        direction,
        entry: entry_macd,
        departure: departure_macd,
        entry_area: a,
        departure_area: b,
        ratio,
        status: if reason.is_none() {
            "ready"
        } else {
            "unavailable"
        }
        .into(),
        reason: reason.map(str::to_owned),
    };
    let result = Evaluation {
        kind: PointKind::First,
        direction,
        status: "confirmed_valid",
        reason: None,
        point: Some(Point {
            kind: PointKind::First,
            direction,
            start: entry_index,
            end,
            centre_start: *start,
            centre_end: end - 1,
            zd: zs.zd,
            zg: zs.zg,
            entry: entry_index,
            departure: end - 1,
            rebound: None,
            retest: None,
            first_point_anchor: Some(end - 1),
            parent_chain: None,
            candidate: false,
            macd_comparison: Some(comparison),
        }),
    };
    if let Some(reason) = reason {
        return result.reject(reason);
    }
    if ratio.unwrap() >= 1. {
        return result.reject("macd_not_weaker");
    }
    // Historical parent anchors are guarded by their second-point path below.
    if end == c.bi_list.len() {
        let boundary = if direction == Direction::Down {
            last.get_low()
        } else {
            last.get_high()
        };
        if let Err(r) = guard_path(c, end, end - 1, direction, boundary, None, true) {
            return result.reject(r);
        }
    }
    result
}

pub fn evaluate_second(
    c: &CZSC,
    end: usize,
    direction: Direction,
    macd: &MacdSeries,
    forming: bool,
) -> Evaluation {
    let miss = |r| Evaluation::miss(PointKind::Second, direction, r);
    let Some(anchor_end) = end.checked_sub(if forming { 1 } else { 2 }) else {
        return miss("missing_material");
    };
    if end > c.bi_list.len() || anchor_end == 0 {
        return miss("missing_material");
    }
    let first = evaluate_first(c, anchor_end, direction, macd);
    if !first.confirmed() {
        return if first.status == "unavailable" {
            miss(first.reason.unwrap_or("missing_coverage"))
        } else {
            miss("no_first_point_anchor")
        };
    }
    let mut p = first.point.unwrap();
    let anchor = anchor_end - 1;
    let rebound = anchor_end;
    if !adjacent(&c.bi_list[anchor], &c.bi_list[rebound]) {
        return miss("invalid_rebound");
    }
    let boundary = if direction == Direction::Down {
        c.bi_list[anchor].get_low()
    } else {
        c.bi_list[anchor].get_high()
    };
    let cap = if direction == Direction::Down {
        c.bi_list[rebound].get_high()
    } else {
        c.bi_list[rebound].get_low()
    };
    if forming {
        if end != c.bi_list.len() || !candidate_matches(c, direction, boundary) {
            return miss("no_current_reverse_candidate");
        }
    } else {
        let last = &c.bi_list[end - 1];
        if !adjacent(&c.bi_list[rebound], last) || last.direction != direction {
            return miss("not_first_retest");
        }
        if if direction == Direction::Down {
            last.get_low() <= boundary
        } else {
            last.get_high() >= boundary
        } {
            return miss("boundary_touched");
        }
    }
    p.kind = PointKind::Second;
    p.end = end;
    p.rebound = Some(rebound);
    p.retest = (!forming).then_some(end - 1);
    p.candidate = forming;
    let result = Evaluation {
        kind: PointKind::Second,
        direction,
        status: if forming {
            "forming_retest"
        } else {
            "confirmed_valid"
        },
        reason: None,
        point: Some(p),
    };
    if let Err(r) = guard_path(c, end, anchor, direction, boundary, None, false) {
        return result.reject(r);
    }
    if let Err(r) = guard_path(c, end, rebound, direction, boundary, Some(cap), false) {
        return result.reject(r);
    }
    result
}

fn candidate_matches(c: &CZSC, direction: Direction, boundary: f64) -> bool {
    let candidate = c.get_candidate_stroke();
    candidate.state == "available"
        && candidate.kind.as_deref() == Some("reverse")
        && candidate.direction == Some(direction)
        && candidate.end_price.is_some_and(|p| {
            if direction == Direction::Down {
                p > boundary
            } else {
                p < boundary
            }
        })
}

pub fn evaluate_third(c: &CZSC, end: usize, direction: Direction, forming: bool) -> Evaluation {
    let miss = |r| Evaluation::miss(PointKind::Third, direction, r);
    if end == 0 || end > c.bi_list.len() {
        return miss("missing_material");
    }
    let finished = c.finished_bis_slice().len().min(end);
    let centres = valid_centres(&c.bi_list[..finished]);
    let Some((start, zs)) = centres.last() else {
        return miss("no_valid_centre");
    };
    let Some(leave_index) = end.checked_sub(if forming { 1 } else { 2 }) else {
        return miss("missing_departure");
    };
    let centre_end = *start + zs.bis.len();
    // A forming departure may be the native last BI not yet included in the
    // finished prefix. Never add that candidate to the canonical sequence.
    let relation = if forming {
        (centre_end == end && zs.bis.len() >= 4) || centre_end == leave_index
    } else {
        centre_end == leave_index + 1 && zs.bis.len() >= 4
    };
    if !relation || leave_index < *start + 3 {
        return miss("not_latest_centre_first_retest");
    }
    if !c.bi_list[start - 1..end]
        .windows(2)
        .all(|w| adjacent(&w[0], &w[1]))
    {
        return miss("invalid_operands");
    }
    let leave = &c.bi_list[leave_index];
    let boundary = if direction == Direction::Down {
        zs.zg
    } else {
        zs.zd
    };
    let departure = finite_bi(leave)
        && if direction == Direction::Down {
            leave.direction == Direction::Up
                && leave.get_low() <= boundary
                && leave.get_high() > boundary
        } else {
            leave.direction == Direction::Down
                && leave.get_high() >= boundary
                && leave.get_low() < boundary
        };
    if !departure {
        return miss("missing_departure");
    }
    if forming {
        if end != c.bi_list.len() || !candidate_matches(c, direction, boundary) {
            return miss("no_current_reverse_candidate");
        }
    } else {
        let last = &c.bi_list[end - 1];
        if !adjacent(leave, last) || last.direction != direction {
            return miss("not_first_retest");
        }
        if if direction == Direction::Down {
            last.get_low() <= boundary
        } else {
            last.get_high() >= boundary
        } {
            return miss("boundary_touched");
        }
    }
    let cap = if direction == Direction::Down {
        leave.get_high()
    } else {
        leave.get_low()
    };
    let result = Evaluation {
        kind: PointKind::Third,
        direction,
        status: if forming {
            "forming_retest"
        } else {
            "confirmed_valid"
        },
        reason: None,
        point: Some(Point {
            kind: PointKind::Third,
            direction,
            start: *start,
            end,
            centre_start: *start,
            centre_end,
            zd: zs.zd,
            zg: zs.zg,
            entry: start - 1,
            departure: leave_index,
            rebound: None,
            retest: (!forming).then_some(end - 1),
            first_point_anchor: None,
            parent_chain: None,
            candidate: forming,
            macd_comparison: None,
        }),
    };
    if let Err(r) = guard_path(c, end, leave_index, direction, boundary, Some(cap), false) {
        return result.reject(r);
    }
    result
}

/// Deliberately finite subset: the new canonical core starts WITH the concrete
/// B2 retest; its entry IS that chain's rebound. After its three seed members,
/// the next rebound and actual retest are required. No label
/// relay, guessed higher-low ancestry, or forming second-like claim.
pub fn evaluate_second_like(
    c: &CZSC,
    end: usize,
    direction: Direction,
    macd: &MacdSeries,
) -> Evaluation {
    let miss = |r| Evaluation::miss(PointKind::SecondLike, direction, r);
    if end == 0 || end > c.bi_list.len() {
        return miss("missing_material");
    }
    let centres = valid_centres(&c.bi_list[..c.finished_bis_slice().len().min(end)]);
    let Some((start, zs)) = centres.last() else {
        return miss("no_valid_centre");
    };
    if *start + 5 != end || *start == 0 || zs.bis.len() < 4 || zs.bis.len() > 5 {
        return miss("no_direct_b2_parent");
    }
    if !c.bi_list[start - 1..end]
        .windows(2)
        .all(|w| adjacent(&w[0], &w[1]))
    {
        return miss("invalid_operands");
    }
    let parent = evaluate_second(c, *start + 1, direction, macd, false);
    if parent.status == "unavailable" {
        // Unknown parent material is not evidence that the finite chain is
        // absent. Preserve its diagnosis without granting a child reference.
        return Evaluation {
            kind: PointKind::SecondLike,
            point: None,
            ..parent
        };
    }
    if !parent.confirmed() {
        return miss("no_direct_b2_parent");
    }
    let parent = parent.point.unwrap();
    let last = &c.bi_list[end - 1];
    let rebound = end - 2;
    if last.direction != direction || !adjacent(&c.bi_list[rebound], last) {
        return miss("not_first_retest");
    }
    let boundary = if direction == Direction::Down {
        zs.zd
    } else {
        zs.zg
    };
    if if direction == Direction::Down {
        last.get_low() <= boundary
    } else {
        last.get_high() >= boundary
    } {
        return miss("boundary_touched");
    }
    let cap = if direction == Direction::Down {
        c.bi_list[rebound].get_high()
    } else {
        c.bi_list[rebound].get_low()
    };
    let result = Evaluation {
        kind: PointKind::SecondLike,
        direction,
        status: "confirmed_valid",
        reason: None,
        point: Some(Point {
            kind: PointKind::SecondLike,
            direction,
            start: *start,
            end,
            centre_start: *start,
            centre_end: *start + zs.bis.len(),
            zd: zs.zd,
            zg: zs.zg,
            entry: start - 1,
            departure: rebound,
            rebound: Some(rebound),
            retest: Some(end - 1),
            first_point_anchor: parent.first_point_anchor,
            parent_chain: Some([
                parent.first_point_anchor.unwrap(),
                parent.rebound.unwrap(),
                parent.retest.unwrap(),
            ]),
            candidate: false,
            // The current core entry/departure are not the parent first-point
            // MACD operands. Their original values remain available in bis.
            macd_comparison: None,
        }),
    };
    if let Err(r) = guard_path(c, end, rebound, direction, boundary, Some(cap), false) {
        return result.reject(r);
    }
    let anchor = parent.first_point_anchor.unwrap();
    let parent_boundary = if direction == Direction::Down {
        c.bi_list[anchor].get_low()
    } else {
        c.bi_list[anchor].get_high()
    };
    if let Err(r) = guard_path(c, end, anchor, direction, parent_boundary, None, false) {
        return result.reject(r);
    }
    result
}

pub fn first_point(c: &CZSC, end: usize, direction: Direction) -> Option<usize> {
    let e = evaluate_first(c, end, direction, &native_macd(c));
    e.confirmed().then(|| {
        let p = e.point.unwrap();
        p.end - p.start
    })
}
pub fn second_point(c: &CZSC, end: usize, direction: Direction) -> bool {
    evaluate_second(c, end, direction, &native_macd(c), false).confirmed()
}
pub type ThirdPoint = Point;
pub fn third_point(c: &CZSC, di: usize) -> Option<ThirdPoint> {
    let end = prefix_end(c, di)?;
    let e = evaluate_third(c, end, c.bi_list[end - 1].direction, false);
    e.confirmed().then(|| e.point.unwrap())
}

//! Buy/sell structure shared by the public CXT signals.
//!
//! These are BI-level conditions, not a recursive decomposition of trends into
//! lower-level completed movements. `di` selects the current BI list; it is not
//! an as-of snapshot. Historical decisions must be replayed bar by bar.

use czsc_core::{
    analyze::{CZSC, utils::get_zs_seq},
    objects::{bi::BI, direction::Direction, zs::ZS},
};

/// Inclusive candidate prefix; never lets a later centre enter an earlier test.
pub fn prefix_end(c: &CZSC, di: usize) -> Option<usize> {
    (di > 0 && di <= c.bi_list.len()).then(|| c.bi_list.len() + 1 - di)
}

fn valid_centres(bis: &[BI]) -> Vec<(usize, ZS)> {
    get_zs_seq(bis)
        .into_iter()
        .filter_map(|zs| {
            if zs.bis.len() < 3 || !zs.is_valid() {
                return None;
            }
            // Entering legs and unformed candidates need not be ZS members.
            // A confirmed outside seed can also move two trailing members.
            let first = zs.bis.first()?;
            let start = bis
                .iter()
                .position(|bi| bi.fx_a.dt == first.fx_a.dt && bi.fx_b.dt == first.fx_b.dt)?;
            Some((start, zs))
        })
        .collect()
}

/// Check the whole rebound/retest path. For a historical offset stop at the
/// candidate endpoint; later BIs/prices must not contaminate that query.
fn holds_since(c: &CZSC, end: usize, start: usize, direction: Direction, boundary: f64) -> bool {
    let begin = c.bi_list[start].fx_b.dt;
    let cutoff = c.bi_list[end - 1].fx_b.dt;
    c.bars_raw
        .iter()
        .filter(|b| b.dt > begin && (end == c.bi_list.len() || b.dt <= cutoff))
        .all(|b| {
            if direction == Direction::Down {
                b.low.is_finite() && b.low > boundary
            } else {
                b.high.is_finite() && b.high < boundary
            }
        })
}

/// First-point auxiliary: at least one canonical BI centre, followed by a new
/// extreme below/above it. Includes consolidation divergence, not just a trend
/// with two separated centres. Compare the actual incoming and outgoing legs,
/// not a maximum chosen from unrelated older legs.
/// Price contraction AND volume-or-duration contraction remain explicit
/// divergence proxies; neither MACD nor a theoretical profit guarantee is implied.
pub fn first_point(c: &CZSC, end: usize, direction: Direction) -> Option<usize> {
    if end < 5 || end > c.bi_list.len() {
        return None;
    }
    let last = &c.bi_list[end - 1];
    if last.direction != direction {
        return None;
    }
    // Exclude the outgoing leg so it cannot change its own trend/background.
    let centres = valid_centres(&c.bi_list[..end - 1]);
    let (start, zs) = centres.last()?;
    // Core construction owns the entry/core distinction. Never choose a
    // different centre phase according to the signal being tested.
    let entry_index = start.checked_sub(1)?;
    let entry = &c.bi_list[entry_index];
    if *start + zs.bis.len() != end - 1 || entry.direction != direction {
        return None;
    }
    let entry_and_extreme = if direction == Direction::Down {
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
    let price = last.get_power_price();
    let entry_price = entry.get_power_price();
    let volume = last.get_power_volume();
    let entry_volume = entry.get_power_volume();
    let weaker = price.is_finite()
        && price > 0.0
        && price < entry_price
        && ((volume.is_finite() && volume > 0.0 && volume < entry_volume)
            || (last.get_length() > 0 && last.get_length() < entry.get_length()));
    // Equal endpoint retests are allowed for BS1; a strictly worse low/high
    // invalidates the current endpoint. BS2/BS3 use strict outer boundaries.
    let endpoint = last.fx_b.dt;
    let intact = end != c.bi_list.len()
        || c.bars_raw.iter().filter(|b| b.dt > endpoint).all(|b| {
            if direction == Direction::Down {
                b.low.is_finite() && b.low >= last.get_low()
            } else {
                b.high.is_finite() && b.high <= last.get_high()
            }
        });
    (entry_and_extreme && weaker && intact).then_some(end - entry_index)
}

/// The very next opposite leg and first pullback after a structural first point.
pub fn second_point(c: &CZSC, end: usize, direction: Direction) -> bool {
    if end < 3 || end > c.bi_list.len() || first_point(c, end - 2, direction).is_none() {
        return false;
    }
    let anchor = &c.bi_list[end - 3];
    let rebound = &c.bi_list[end - 2];
    let last = &c.bi_list[end - 1];
    if last.direction != direction || rebound.direction == direction {
        return false;
    }
    let boundary = if direction == Direction::Down {
        anchor.get_low()
    } else {
        anchor.get_high()
    };
    let holds = if direction == Direction::Down {
        last.get_low() > boundary
    } else {
        last.get_high() < boundary
    };
    holds && holds_since(c, end, end - 3, direction, boundary)
}

pub struct ThirdPoint {
    pub start: usize,
    pub end: usize,
    pub zd: f64,
    pub zg: f64,
}

/// Latest valid finished canonical centre and its immediate departure/retest.
/// An incomplete 1/2-BI tail is not a new centre. Once a newer centre exists,
/// earlier centres cannot be selected. The departure is often the last member
/// of the old centre, so comparing its high to that centre's GG is circular.
pub fn third_point(c: &CZSC, di: usize) -> Option<ThirdPoint> {
    let end = prefix_end(c, di)?;
    if end < 5 {
        return None;
    }
    let finished = c.finished_bis_slice().len().min(end);
    let centres = valid_centres(&c.bi_list[..finished]);
    let (start, zs) = centres.last()?;
    // At least three centre BIs precede the departure. Its next BI is the
    // candidate; an outside second retest would no longer be adjacent to ZS.
    if zs.bis.len() < 4 || *start + zs.bis.len() != end - 1 {
        return None;
    }
    let leave = &c.bi_list[end - 2];
    let last = &c.bi_list[end - 1];
    let valid = match last.direction {
        Direction::Down => {
            leave.direction == Direction::Up
                && leave.get_low() <= zs.zg
                && leave.get_high() > zs.zg
                && last.get_low() > zs.zg
                && holds_since(c, end, end - 2, Direction::Down, zs.zg)
        }
        Direction::Up => {
            leave.direction == Direction::Down
                && leave.get_high() >= zs.zd
                && leave.get_low() < zs.zd
                && last.get_high() < zs.zd
                && holds_since(c, end, end - 2, Direction::Up, zs.zd)
        }
    };
    valid.then_some(ThirdPoint {
        start: *start,
        end,
        zd: zs.zd,
        zg: zs.zg,
    })
}

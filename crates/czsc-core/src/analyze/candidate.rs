//! Display-only tail geometry from inclusion-processed NewBars. This never
//! calls or reimplements check_bi, and does not claim that a new BI exists.
use super::CZSC;
use crate::objects::{bar::NewBar, direction::Direction};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateStroke {
    pub state: String,
    pub reason: Option<String>,
    pub kind: Option<String>,
    pub direction: Option<Direction>,
    /// Natural endpoint of the native anchor FX, not its observation time.
    pub anchor_at: Option<DateTime<Utc>>,
    pub anchor_price: Option<f64>,
    pub anchor_price_at: Option<DateTime<Utc>>,
    pub end_price: Option<f64>,
    pub end_price_at: Option<DateTime<Utc>>,
    pub end_newbar_at: Option<DateTime<Utc>>,
    pub as_of: Option<DateTime<Utc>>,
    /// Only the signal owner can assign first_retest_candidate.
    pub role: String,
}

fn boundary(bar: &NewBar, up: bool) -> f64 {
    if up { bar.high } else { bar.low }
}

impl CZSC {
    pub fn get_last_bi_extend(&self) -> bool {
        self.bi_list.last().is_some_and(|bi| {
            self.bars_ubi.iter().any(|b| match bi.direction {
                Direction::Up => b.high > bi.get_high(),
                Direction::Down => b.low < bi.get_low(),
            })
        })
    }

    pub fn get_candidate_stroke(&self) -> CandidateStroke {
        let mut result = CandidateStroke {
            state: "none".into(),
            reason: Some("no_identified_stroke".into()),
            kind: None,
            direction: None,
            anchor_at: None,
            anchor_price: None,
            anchor_price_at: None,
            end_price: None,
            end_price_at: None,
            end_newbar_at: None,
            as_of: self.bars_raw.last().map(|b| b.dt),
            role: "other".into(),
        };
        let Some(bi) = self.bi_list.last() else {
            return result;
        };
        let anchor = &bi.fx_b;
        let up = bi.direction == Direction::Up;
        result.anchor_at = Some(anchor.dt);
        result.anchor_price = anchor.fx.is_finite().then_some(anchor.fx);
        let owners: Vec<_> = anchor
            .elements
            .iter()
            .flat_map(|b| &b.elements)
            .filter(|b| (if up { b.high } else { b.low }) == anchor.fx)
            .collect();
        let owner = owners.iter().find(|b| b.dt == anchor.dt).or(owners.first());
        let Some(owner) = owner else {
            result.state = "unavailable".into();
            result.reason = Some("anchor_price_source_missing".into());
            return result;
        };
        result.anchor_price_at = Some(owner.dt);
        let central = self
            .bars_ubi
            .iter()
            .position(|b| b.dt == anchor.dt || b.elements.iter().any(|r| r.dt == anchor.dt));
        let Some(central) = central else {
            result.state = "unavailable".into();
            result.reason = Some("anchor_not_in_engine_tail".into());
            return result;
        };
        let extension = self.get_last_bi_extend();
        let forward_up = if extension { up } else { !up };
        result.kind = Some(if extension { "extension" } else { "reverse" }.into());
        result.direction = Some(if forward_up {
            Direction::Up
        } else {
            Direction::Down
        });
        let eligible = &self.bars_ubi[central + usize::from(!extension)..];
        if !anchor.fx.is_finite()
            || eligible
                .iter()
                .any(|b| !b.high.is_finite() || !b.low.is_finite())
        {
            result.state = "unavailable".into();
            result.reason = Some("non_finite".into());
            return result;
        }
        // Stable first extreme; select NewBar boundaries, only then locate the
        // raw owner of that exact price. Raw bars never choose the geometry.
        let end = eligible
            .iter()
            .filter(|b| {
                if forward_up {
                    b.high > anchor.fx
                } else {
                    b.low < anchor.fx
                }
            })
            .reduce(|a, b| {
                if if forward_up {
                    b.high > a.high
                } else {
                    b.low < a.low
                } {
                    b
                } else {
                    a
                }
            });
        let Some(end) = end else {
            result.reason = Some("no_forward_extreme".into());
            return result;
        };
        let price = boundary(end, forward_up);
        let owners: Vec<_> = end
            .elements
            .iter()
            .filter(|b| b.dt > anchor.dt && (if forward_up { b.high } else { b.low }) == price)
            .collect();
        let owner = owners.iter().find(|b| b.dt == end.dt).or(owners.first());
        let Some(owner) = owner else {
            result.state = "unavailable".into();
            result.reason = Some("no_forward_price_source".into());
            return result;
        };
        if !self
            .bars_raw
            .iter()
            .any(|b| b.dt == owner.dt && (if forward_up { b.high } else { b.low }) == price)
            || !self
                .bars_raw
                .iter()
                .any(|b| Some(b.dt) == result.anchor_price_at)
        {
            result.state = "unavailable".into();
            result.reason = Some("missing_coverage".into());
            return result;
        }
        result.state = "available".into();
        result.reason = None;
        result.end_price = Some(price);
        result.end_price_at = Some(owner.dt);
        result.end_newbar_at = Some(end.dt);
        result
    }
}

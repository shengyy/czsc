use chrono::{TimeZone, Utc};
use czsc_core::{
    analyze::CZSC,
    objects::{bar::RawBarBuilder, direction::Direction, freq::Freq},
};

fn bars() -> Vec<czsc_core::objects::bar::RawBar> {
    let endpoints = [
        20., 10., 18., 16., 21., 12., 19., 17., 23., 14., 20., 18., 25., 16., 24., 20.,
    ];
    let mut out = Vec::new();
    for (leg, p) in endpoints.windows(2).enumerate() {
        let count = if leg % 3 == 2 { 1 } else { 12 };
        for step in 0..count {
            let price = p[0] + (p[1] - p[0]) * step as f64 / count as f64;
            out.push(
                RawBarBuilder::default()
                    .symbol("MOCK")
                    .id(out.len() as i32)
                    .freq(Freq::D)
                    .dt(Utc
                        .timestamp_opt(1700000000 + out.len() as i64 * 86400, 0)
                        .unwrap())
                    .open(price)
                    .close(price)
                    .high(price + 0.1)
                    .low(price - 0.1)
                    .vol(1.)
                    .amount(price)
                    .build()
                    .unwrap(),
            );
        }
    }
    out
}

#[test]
fn native_tail_candidate_prefix_revision_and_raw_price_ownership() {
    let bars = bars();
    let mut c = CZSC::new(vec![bars[0].clone()], 50, 6);
    let mut available = 0;
    let mut withdrawn = 0;
    let mut fresh_after_withdrawal = 0;
    for (i, b) in bars.iter().enumerate().skip(1) {
        let prior = c.bi_list.len();
        c.update_bar(b.clone());
        if c.bi_list.len() < prior {
            withdrawn += 1;
        }
        if withdrawn > 0 && c.bi_list.len() > prior {
            fresh_after_withdrawal += 1;
        }
        let s = c.get_candidate_stroke();
        let cold = CZSC::new(bars[..=i].to_vec(), 50, 6);
        assert_eq!(s, cold.get_candidate_stroke());
        if s.state == "available" {
            available += 1;
            if available == 1 {
                let mut missing = c.clone();
                missing.bars_raw.retain(|b| Some(b.dt) != s.end_price_at);
                let unavailable = missing.get_candidate_stroke();
                assert_eq!(unavailable.state, "unavailable");
                assert_eq!(unavailable.reason.as_deref(), Some("missing_coverage"));
            }

            let nb = c
                .bars_ubi
                .iter()
                .find(|b| Some(b.dt) == s.end_newbar_at)
                .unwrap();
            let field = |high, low| {
                if s.direction == Some(Direction::Up) {
                    high
                } else {
                    low
                }
            };
            assert_eq!(s.end_price, Some(field(nb.high, nb.low)));
            assert!(
                nb.elements
                    .iter()
                    .any(|r| Some(r.dt) == s.end_price_at
                        && Some(field(r.high, r.low)) == s.end_price)
            );
        }
        // Repeated same-dt revisions are exact replacements, including after
        // a BI was withdrawn. Candidate metadata must not leak prior versions.
        if i % 11 == 0 {
            let mut revised = b.clone();
            revised.high += 2.;
            c.update_bar(revised.clone());
            let mut revised_prefix = bars[..i].to_vec();
            revised_prefix.push(revised);
            assert_eq!(
                c.get_candidate_stroke(),
                CZSC::new(revised_prefix, 50, 6).get_candidate_stroke()
            );
            c.update_bar(b.clone());
            assert_eq!(c.get_candidate_stroke(), s);
        }
    }
    assert!(available > 20);
    assert!(
        withdrawn > 0 && fresh_after_withdrawal > 0,
        "withdrawn={withdrawn}, fresh={fresh_after_withdrawal}"
    );
}

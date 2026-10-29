"""Independent-review reproduction through the installed native wheel only."""

import json

import pytest

import czsc
from czsc._native.signals import call_signal
from czsc.mock import generate_symbol_kines


@pytest.fixture(scope="module", params=[False, True], ids=["buy", "sell"])
def case(request):
    frame = generate_symbol_kines("000001", "30分钟", "20230101", "20240101", seed=7)
    if request.param:
        frame[["open", "close"]] = 1000 - frame[["open", "close"]]
        frame["high"], frame["low"] = 1000 - frame["low"], 1000 - frame["high"]
    return czsc.format_standard_kline(frame, freq=czsc.Freq.F30)[:982], "Up" if request.param else "Down"


def views(c, di=1):
    full, light = czsc.get_structure(c, di), czsc.get_structure_state(c, di)
    assert light == {key: full[key] for key in light}
    # All five existing public CXT entries still share these exact results.
    for name, kind, side in [
        ("cxt_first_buy_V221126", "first", "Down"),
        ("cxt_first_sell_V221126", "first", "Up"),
        ("cxt_second_bs_V230320", "second", None),
        ("cxt_third_buy_V230228", "third", "Down"),
        ("cxt_third_bs_V230318", "third", None),
    ]:
        expected = any(
            r["kind"] == kind and (side is None or r["direction"] == side) and r["status"] == "confirmed_valid"
            for r in light["results"]
        )
        assert (not call_signal(name, c, {"di": di})[0].value.startswith("其他")) == expected
    return light


def pick(c, di, kind, direction):
    return next(r for r in views(c, di)["results"] if r["kind"] == kind and r["direction"] == direction)


@pytest.mark.parametrize("cap", [25, 30, 50])
def test_public_retention_changes_parent_readiness_not_parent_existence(case, cap):
    rows, direction = case
    c = czsc.CZSC(rows, min_bi_len=6, max_bi_num=cap)
    parent = pick(c, 5, "second", direction)
    child = pick(c, 1, "second_like", direction)
    expected = ("unavailable", "warmup") if cap == 25 else ("confirmed_valid", None)
    assert (parent["status"], parent["reason"]) == expected
    assert (child["status"], child["reason"]) == expected
    if cap == 25:
        assert child["reference"] is None
    else:
        assert child["reference"]["b2_chain"]["first_retest"] == parent["reference"]["retest"]

    # A direction-mismatched parent is determinately absent, not unavailable.
    opposite = "Up" if direction == "Down" else "Down"
    assert pick(c, 5, "second", opposite)["status"] == "no_match"
    no_parent = pick(c, 1, "second_like", opposite)
    assert (no_parent["status"], no_parent["reason"]) == ("no_match", "no_direct_b2_parent")


@pytest.mark.parametrize("reason", ["missing_coverage", "zero_entry_area"])
def test_parent_material_faults_remain_unavailable_in_both_views(case, reason):
    rows, direction = case
    c = czsc.CZSC(rows, min_bi_len=6, max_bi_num=50)
    original = views(c)
    entry = pick(c, 7, "first", direction)["reference"]["entry"]
    saved = c.__getstate__()
    state = json.loads(saved)
    # Deliberate numerical/coverage fault injection into a native snapshot;
    # retain its BI geometry to isolate availability, not simulate new quotes.
    if reason == "missing_coverage":
        index = next(i for i, b in enumerate(state["bars_raw"]) if entry["sdt"] < b["dt"] < entry["edt"])
        del state["bars_raw"][index]
    else:
        for bar in state["bars_raw"]:
            bar["close"] = 500.0
    c.__setstate__(json.dumps(state).encode())
    parent = pick(c, 5, "second", direction)
    child = pick(c, 1, "second_like", direction)
    assert (parent["status"], parent["reason"]) == ("unavailable", reason)
    assert (child["status"], child["reason"]) == (parent["status"], parent["reason"])
    assert child["reference"] is None
    c.__setstate__(saved)
    assert views(c) == original


@pytest.mark.parametrize("cap", [25, 50])
def test_parent_chain_same_dt_revision_matches_cold_prefix_and_restores(case, cap):
    rows, _ = case
    c = czsc.CZSC(rows, min_bi_len=6, max_bi_num=cap)
    original = views(c)
    last = rows[-1]
    revised = czsc.RawBar(
        symbol=last.symbol,
        id=last.id,
        freq=last.freq,
        dt=last.dt,
        open=last.open,
        close=last.close,
        high=last.high + 2,
        low=last.low - 2,
        vol=last.vol,
        amount=last.amount,
    )
    c.update(revised)
    cold = czsc.CZSC(rows[:-1] + [revised], min_bi_len=6, max_bi_num=cap)
    for di in (1, 5, 7):
        assert views(c, di) == views(cold, di)
    c.update(last)
    assert views(c) == original

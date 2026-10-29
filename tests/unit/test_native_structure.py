"""Native owner contract through the installed wheel; inputs use czsc.mock."""

import copy
import json

import pandas as pd
import pytest

import czsc
from czsc._native.signals import call_signal
from czsc.mock import generate_symbol_kines


@pytest.fixture(scope="module")
def bars():
    return czsc.format_standard_kline(
        generate_symbol_kines("000001", "30分钟", "20230101", "20240101", seed=42), freq=czsc.Freq.F30
    )


def timestamp(value):
    return pd.Timestamp(value).tz_localize(None)


def eligible(snapshot):
    return [r for r in snapshot["results"] if r["status"] in ("confirmed_valid", "forming_retest")]


STATE_FIELDS = {
    "basis",
    "di",
    "as_of",
    "status",
    "previous_zs",
    "latest_finished_zs",
    "zs_candidate",
    "candidate_stroke",
    "results",
}


def read_structure(c, di=1):
    """Compare every light field with full, including failed operand references."""
    full = czsc.get_structure(c, di=di)
    light = czsc.get_structure_state(c, di=di)
    assert set(light) == STATE_FIELDS
    assert light == {key: full[key] for key in STATE_FIELDS}
    json.dumps(light, allow_nan=False)
    return full


def test_public_function_is_native_and_missing_material_is_explicit(bars):
    assert czsc.get_structure is czsc._native.get_structure
    assert czsc.get_structure_state is czsc._native.get_structure_state
    c = czsc.CZSC(bars[:10])
    s = read_structure(c)
    assert s["status"] == "missing_material"
    assert s["basis"] == "bi_proxy"
    assert s["macd"]["hist_scale"] == 1
    assert s["macd"]["params"] == [12, 26, 9]
    assert s["macd"]["warmup_bars"] == 33
    assert all(row["hist"] is None for row in s["macd"]["bars"])
    assert not s["results"]
    json.dumps(s, allow_nan=False)


def test_stream_geometry_roles_signals_and_forming_are_the_same_owner(bars):
    c = czsc.CZSC(bars[:100], max_bi_num=50)
    stages, families, tails = set(), set(), set()
    for bar in bars[100:]:
        c.update(bar)
        s = read_structure(c)
        zones = [z for z in c.zs_list if len(z.bis) >= 3 and z.is_valid()]
        for key, offset in [("latest_finished_zs", 1), ("previous_zs", 2)]:
            if len(zones) >= offset:
                actual = s[key]
                expected = zones[-offset]
                assert [actual[k] for k in ("zd", "zg", "dd", "gg")] == [
                    expected.zd,
                    expected.zg,
                    expected.dd,
                    expected.gg,
                ]
                assert actual["bi_count"] == len(expected.bis)
                assert actual["members"][0]["sdt"] == actual["sdt"]
            else:
                assert s[key] is None
        if s["zs_candidate"]:
            tails.add(s["zs_candidate"]["bi_count"])
            assert not s["zs_candidate"]["valid"]
        for r in eligible(s):
            stages.add(r["status"])
            families.add(r["kind"])
            ref = r["reference"]
            assert ref["basis"] == "bi_proxy"
            assert ref["stage"] == r["status"]
            if r["status"] == "forming_retest":
                assert r["kind"] in ("second", "third")
                assert ref["retest"] is None
                assert ref["candidate_stroke"]["kind"] == "reverse"
                assert ref["candidate_stroke"]["role"] == "first_retest_candidate"
            else:
                assert ref["candidate_stroke"] is None
            if ref["macd_comparison"]:
                comparison = ref["macd_comparison"]
                assert comparison["status"] == "ready"
                assert comparison["entry_area"] > comparison["departure_area"] >= 0
            # The declared natural interval must point into the same native list.
            for role in ("entry", "departure", "rebound", "retest", "first_point_anchor"):
                operand = ref[role]
                if operand:
                    bi = c.bi_list[operand["position"] - 1]
                    assert timestamp(operand["sdt"]) == bi.sdt
                    assert timestamp(operand["edt"]) == bi.edt
                    assert (operand["high"], operand["low"]) == (bi.high, bi.low)
        for name, kind, side in [
            ("cxt_first_buy_V221126", "first", "Down"),
            ("cxt_first_sell_V221126", "first", "Up"),
            ("cxt_second_bs_V230320", "second", None),
            ("cxt_third_bs_V230318", "third", None),
            ("cxt_third_buy_V230228", "third", "Down"),
        ]:
            expected = any(
                r["kind"] == kind and (side is None or r["direction"] == side) and r["status"] == "confirmed_valid"
                for r in s["results"]
            )
            result = call_signal(name, c, {"timeperiod": 100000})[0].value
            assert (not result.startswith("其他")) == expected
    assert stages == {"confirmed_valid", "forming_retest"}
    assert {"first", "second", "third"} <= families
    assert tails == {1, 2}


def test_per_bi_macd_is_exactly_the_published_raw_interval(bars):
    c = czsc.CZSC(bars[:1400], max_bi_num=100)
    s = read_structure(c)
    raw = s["macd"]["bars"]
    assert not s["macd"]["truncated"]
    assert len(raw) == len(c.bars_raw)
    assert all(r["hist"] is None for r in raw[:33])
    assert all(r["hist"] is not None for r in raw[33:])
    for row in raw[33:]:
        assert row["hist"] == row["dif"] - row["dea"]
    ready = 0
    for b in s["bis"]:
        power = b["power"]
        macd = power["macd"]
        interval = [r for r in raw if b["sdt"] < r["dt"] <= b["edt"]]
        assert macd["sample_count"] == len(interval)
        assert macd["ready_count"] == sum(r["hist"] is not None for r in interval)
        if macd["status"] == "ready":
            ready += 1
            assert macd["positive_area"] == pytest.approx(sum(max(r["hist"], 0) for r in interval), abs=1e-12)
            assert macd["negative_area"] == pytest.approx(sum(max(-r["hist"], 0) for r in interval), abs=1e-12)
            assert macd["hist_at_end"] == interval[-1]["hist"]
        else:
            assert macd["reason"] in ("warmup", "missing_coverage", "non_finite")
            assert macd["positive_area"] is macd["negative_area"] is None
        assert b["included_in_finished_bis"] == b["finished"]
    assert ready > 3
    json.dumps(s, allow_nan=False)


def test_prefix_replay_revision_copy_and_detached_results(bars):
    c = czsc.CZSC(bars[:100], max_bi_num=8)
    for i, bar in enumerate(bars[100:500], 100):
        c.update(bar)
        if i % 41:
            continue
        original = read_structure(c)
        assert original == read_structure(czsc.CZSC(bars[: i + 1], max_bi_num=8))
        clone = copy.deepcopy(c)
        revised = czsc.RawBar(
            symbol=bar.symbol,
            id=bar.id,
            freq=bar.freq,
            dt=bar.dt,
            open=bar.open,
            close=bar.close,
            high=bar.high * 1.02,
            low=bar.low * 0.98,
            vol=bar.vol,
            amount=bar.amount,
        )
        c.update(revised)
        expected = czsc.CZSC(bars[:i] + [revised], max_bi_num=8)
        assert read_structure(c) == read_structure(expected)
        c.update(bar)
        assert read_structure(c) == original == read_structure(clone)
        snapshot = read_structure(c)
        snapshot["results"].clear()
        snapshot["macd"]["bars"].clear()
        assert read_structure(c) == original
        sliced = read_structure(c, di=2)
        assert sliced["candidate_stroke"]["reason"] == "di_is_not_history"
        assert all(r["status"] != "forming_retest" for r in sliced["results"])


def test_signal_references_and_public_snapshot_share_first_second_operands(bars):
    names = ["cxt_first_buy_V221126", "cxt_first_sell_V221126", "cxt_second_bs_V230320"]
    bg = czsc.BarGenerator(base_freq="30分钟", freqs=[], max_count=10000)
    for bar in bars[:200]:
        bg.update(bar)
    cs = czsc.CzscSignals(bg, [{"name": name, "freq": "30分钟"} for name in names])
    seen = set()
    for bar in bars[200:]:
        cs.update_signals(bar)
        refs = cs.signal_references
        assert set(refs) == {k for k, v in cs.s.items() if v.startswith(("一买_", "一卖_", "二买_", "二卖_"))}
        if not refs:
            continue
        s = read_structure(cs.kas["30分钟"])
        for ref in refs.values():
            structure = ref["structure"]
            seen.add(structure["kind"])
            result = next(
                r for r in s["results"] if r["kind"] == structure["kind"] and r["direction"] == structure["direction"]
            )
            assert result["status"] == "confirmed_valid"
            assert result["reference"] == structure
        if seen == {"first", "second"}:
            break
    assert seen == {"first", "second"}


def test_second_like_has_direct_native_parent_and_current_core_roles():
    # Reproducible mock prefix found by the offline seed probe, not real quotes.
    rows = czsc.format_standard_kline(
        generate_symbol_kines("000001", "30分钟", "20230101", "20250101", seed=7), freq=czsc.Freq.F30
    )
    c = czsc.CZSC(rows[:982], max_bi_num=50, min_bi_len=6)
    snapshot = read_structure(c)
    result = next(r for r in snapshot["results"] if r["kind"] == "second_like" and r["status"] == "confirmed_valid")
    reference = result["reference"]
    chain = reference["b2_chain"]
    assert reference["centre"]["members"][0] == chain["first_retest"]
    assert reference["centre"]["entry"] == chain["rebound"]
    assert reference["departure"] == reference["rebound"]
    assert reference["macd_comparison"] is None  # parent comparison is not a comparison of these new-core roles
    assert reference["candidate_stroke"] is None
    assert chain["first_point_anchor"]["edt"] == chain["rebound"]["sdt"]
    assert chain["rebound"]["edt"] == chain["first_retest"]["sdt"]
    last = rows[981]
    buy = result["direction"] == "Down"
    boundary = reference["centre"]["zd" if buy else "zg"]
    c.update(
        czsc.RawBar(
            symbol=last.symbol,
            id=last.id,
            freq=last.freq,
            dt=last.dt,
            open=last.open,
            close=last.close,
            high=last.high if buy else max(boundary, last.high),
            low=min(boundary, last.low) if buy else last.low,
            vol=last.vol,
            amount=last.amount,
        )
    )
    assert not any(
        r["kind"] == "second_like" and r["status"] == "confirmed_valid" for r in read_structure(c)["results"]
    )
    c.update(last)
    assert read_structure(c) == snapshot


def test_daily_research_stages_use_the_same_signal_owner():
    rows = czsc.format_standard_kline(
        generate_symbol_kines("000001", "日线", "20100101", "20250101", seed=7), freq=czsc.Freq.D
    )
    c = czsc.CZSC(rows[:100])
    stages = set()
    for bar in rows[100:]:
        c.update(bar)
        snapshot = read_structure(c)
        stages.update(r["status"] for r in eligible(snapshot))
        expected = any(
            r["kind"] == "third" and r["status"] == "confirmed_valid" and r["direction"] == "Down"
            for r in snapshot["results"]
        )
        assert (not call_signal("cxt_third_buy_V230228", c)[0].value.startswith("其他")) == expected
        if stages == {"confirmed_valid", "forming_retest"}:
            break
    assert stages == {"confirmed_valid", "forming_retest"}


def test_light_full_missing_coverage_retains_same_reason_and_operands(bars):
    c = czsc.CZSC(bars[:100], max_bi_num=50, min_bi_len=6)
    for bar in bars[100:]:
        c.update(bar)
        snapshot = read_structure(c)
        first = next(
            (r for r in snapshot["results"] if r["kind"] == "first" and r["status"] == "confirmed_valid"), None
        )
        if first is not None:
            break
    assert first is not None
    entry = first["reference"]["entry"]
    # Remove one raw source row from the native serde snapshot while retaining
    # its BI/NewBar evidence. This simulates incomplete coverage, not geometry.
    saved = c.__getstate__()
    state = json.loads(saved)
    index = next(i for i, b in enumerate(state["bars_raw"]) if entry["sdt"] < b["dt"] < entry["edt"])
    del state["bars_raw"][index]
    c.__setstate__(json.dumps(state).encode())
    missing = read_structure(c)
    actual = next(r for r in missing["results"] if (r["kind"], r["direction"]) == ("first", first["direction"]))
    assert (actual["status"], actual["reason"]) == ("unavailable", "missing_coverage")
    assert actual["reference"]["entry"] == entry
    assert actual["reference"]["macd_comparison"]["entry_area"] is None
    c.__setstate__(saved)
    assert read_structure(c) == snapshot
    light = czsc.get_structure_state(c)
    light["results"].clear()
    light["latest_finished_zs"]["members"].clear()
    assert read_structure(c) == snapshot

"""Current native hit operands; bars come exclusively from reproducible czsc.mock data."""

import copy
import pickle

import pandas as pd
import pytest

from czsc import BarGenerator, CzscSignals, Freq, RawBar, format_standard_kline, get_zs_seq
from czsc.mock import generate_symbol_kines

NAMES = ("cxt_third_buy_V230228", "cxt_third_bs_V230318")
CONFIGS = [{"name": name, "freq": "30分钟", "di": di} for name in NAMES for di in (1, 2)]


@pytest.fixture(scope="module")
def bars():
    return format_standard_kline(
        generate_symbol_kines("000001", "30分钟", "20230101", "20250101", seed=42), freq=Freq.F30
    )


def engine(bars, configs=CONFIGS):
    bg = BarGenerator(base_freq="30分钟", freqs=[], max_count=10000)
    for bar in bars:
        bg.update(bar)
    return CzscSignals(bg, configs)


def assert_bi(fact, bi, finished):
    assert fact["direction"] == ("Up" if str(bi.direction) == "向上" else "Down")
    assert pd.Timestamp(fact["sdt"]).tz_localize(None) == bi.sdt
    assert pd.Timestamp(fact["edt"]).tz_localize(None) == bi.edt
    assert fact["high"] == bi.high
    assert fact["low"] == bi.low
    assert fact["finished"] == finished


def assert_references(cs):
    refs = cs.signal_references
    assert set(refs) == {key for key, value in cs.s.items() if value.startswith(("三买_", "三卖_"))}
    if not refs:
        return
    c = cs.kas["30分钟"]
    bis = c.bi_list
    finished = len(c.finished_bis)
    for key, r in refs.items():
        assert set(r) == {"kind", "window", "overlap_bis", "last_bi", "zd", "zg", "structure"}
        assert r["structure"]["basis"] == "bi_proxy"
        assert r["structure"]["stage"] == "confirmed_valid"
        w = r["window"]
        end = len(bis) + 1 - w["di"]
        start = end - w["n"]
        assert r["last_bi"]["position"] == w["n"]
        for b in [*r["overlap_bis"], r["last_bi"]]:
            index = start + b["position"] - 1
            assert_bi(b, bis[index], index < finished)
        assert r["zd"] == max(b["low"] for b in r["overlap_bis"])
        assert r["zg"] == min(b["high"] for b in r["overlap_bis"])
        assert r["kind"] == "canonical_zs"
        assert [b["position"] for b in r["overlap_bis"]] == [1, 2, 3]
        centres = [z for z in get_zs_seq(bis[: min(end, finished)]) if len(z.bis) >= 3 and z.is_valid()]
        assert r["zd"] == centres[-1].zd
        assert r["zg"] == centres[-1].zg
        if "三买辅助" in key:
            assert cs.s[key].split("_")[1] == f"{w['n']}笔"
        assert r["zd"] <= r["zg"]


def test_stream_current_content_same_value_changes_and_disappearance(bars):
    cs = engine(bars[:300])
    previous = {}
    previous_values = {}
    seen, changed, disappeared, unfinished, historical_finished = set(), set(), set(), set(), set()
    for bar in bars[300:]:
        cs.update_signals(bar)
        assert_references(cs)
        current = cs.signal_references
        for key, r in current.items():
            seen.add(key)
            if key in previous and previous_values[key] == cs.s[key] and previous[key] != r:
                changed.add(key)
            if not r["last_bi"]["finished"]:
                unfinished.add(key)
            if r["window"]["di"] == 2 and r["last_bi"]["finished"]:
                historical_finished.add(key)
        disappeared.update(previous.keys() - current.keys())
        previous = current
        previous_values = cs.s
    assert len(seen) == len(disappeared) == 4
    assert changed == {key for key in seen if "_D1" in key}
    assert len(unfinished) == len(historical_finished) == 2


@pytest.fixture(scope="module")
def hit_cases(bars):
    cs = engine(bars[:300])
    found = {}
    for i, bar in enumerate(bars[300:], 300):
        cs.update_signals(bar)
        for key in cs.signal_references:
            family = "buy" if "三买辅助" in key else "bs"
            if family not in found:
                found[family] = i
        if len(found) == 2:
            break
    assert len(found) == 2
    return found


@pytest.mark.parametrize("family", ["buy", "bs"])
def test_tail_revision_replay_and_unchanged_frequency(bars, hit_cases, family):
    i = hit_cases[family]
    cs = engine(bars[:300])
    for bar in bars[300 : i + 1]:
        cs.update_signals(bar)
    original = cs.signal_references
    assert original
    cs.update_signals(bars[i])
    assert cs.signal_references == original  # unchanged frequency reuse
    # The returned nested dict is a detached snapshot.
    snapshot = cs.signal_references
    snapshot[next(iter(snapshot))]["last_bi"]["low"] = -1
    assert cs.signal_references == original
    snapshot.clear()
    assert cs.signal_references == original
    bar = bars[i]
    revised = RawBar(
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
    cs.update_signals(revised)
    # Duplicate base timestamps are intentionally ignored by BarGenerator.
    assert cs.signal_references == original
    assert_references(cs)
    cs.update_signals(bar)
    assert cs.signal_references == original


@pytest.mark.parametrize("restore", [copy.copy, copy.deepcopy, lambda c: pickle.loads(pickle.dumps(c))])
def test_pickle_copy_preserves_existing_fresh_rebuild_contract(bars, hit_cases, restore):
    i = hit_cases["buy"]
    cs = engine(bars[:i])
    cs.update_signals(bars[i])
    assert cs.signal_references
    rebuilt = restore(cs)
    assert rebuilt.s == {}
    assert rebuilt.signal_references == {}
    fresh = CzscSignals(*cs.__reduce__()[1])
    for c in (rebuilt, fresh):
        c.update_signals(bars[i])
    assert rebuilt.s == fresh.s
    assert rebuilt.signal_references == fresh.signal_references
    assert_references(rebuilt)


def test_rebuild_configs_and_frequency_do_not_leak_old_hits(bars, hit_cases):
    i = hit_cases["buy"]
    cs = engine(bars[:i])
    cs.update_signals(bars[i])
    original = cs.signal_references
    assert original
    bg, configs = cs.__reduce__()[1]
    configs.clear()
    empty = CzscSignals(bg, configs)
    empty.update_signals(bars[i])
    assert empty.signal_references == {}
    assert cs.signal_references == original
    for config in ([{"name": NAMES[0], "freq": "日线"}], [{"name": "cxt_bi_base_V230228", "freq": "30分钟"}]):
        other = CzscSignals(bg, config)
        other.update_signals(bars[i])
        assert other.signal_references == {}
    # Config properties are copies; modifying them cannot mutate the native plan.
    cs.signals_config.clear()
    cs.update_signals(bars[i])
    assert cs.signal_references == original
    short = engine(bars[:10])
    short.update_signals(bars[10])
    assert short.signal_references == {}


def test_aggregated_tail_revisions_refresh_current_operands():
    bars = format_standard_kline(
        generate_symbol_kines("000001", "5分钟", "20230101", "20240101", seed=42), freq=Freq.F5
    )
    bg = BarGenerator(base_freq="5分钟", freqs=["30分钟"], max_count=10000)
    for bar in bars[:600]:
        bg.update(bar)
    cs = CzscSignals(bg, CONFIGS)
    previous, previous_values, previous_dt = {}, {}, None
    changed = set()
    for bar in bars[600:]:
        cs.update_signals(bar)
        current = cs.signal_references
        structure = cs.kas["30分钟"]
        dt = structure.bars_raw[-1].dt
        if current and dt == previous_dt:
            assert_references(cs)
            for key, r in current.items():
                if key in previous and r != previous[key] and cs.s[key] == previous_values[key]:
                    changed.add("buy" if "三买辅助" in key else "bs")
        previous, previous_values, previous_dt = current, cs.s, dt
    assert changed == {"buy", "bs"}

"""Same native reads through CzscSignals, without copying its frequency engines."""

import copy
import pickle

import pytest

import czsc
from czsc.mock import generate_symbol_kines


@pytest.fixture(scope="module")
def bars():
    return czsc.format_standard_kline(
        generate_symbol_kines("000001", "30分钟", "20230101", "20240101", seed=42), freq=czsc.Freq.F30
    )


def engine(bars):
    bg = czsc.BarGenerator("30分钟", ["日线"], max_count=200)
    for bar in bars:
        bg.update(bar)
    return czsc.CzscSignals(
        bg,
        [{"name": "cxt_third_bs_V230318", "freq": freq, "di": 1} for freq in ("30分钟", "日线")],
    )


def check_reads(analysis):
    for freq, native in analysis.kas.items():
        for di in (0, 1, 2):
            actual = analysis.structure_state(freq, di=di)
            expected = czsc.get_structure_state(native, di=di)
            assert actual == expected
            assert list(actual) == list(expected)
            actual["results"].clear()
            assert analysis.structure_state(freq, di=di) == expected
        last = analysis.last_bi(freq)
        bis = native.bi_list
        assert last == (bis[-1] if bis else None)


def test_stream_reads_and_missing_frequency_are_explicit(bars):
    analysis = engine(bars[:1])
    assert analysis.last_bi("30分钟") is None
    check_reads(analysis)
    for method in (analysis.structure_state, analysis.last_bi):
        with pytest.raises(ValueError, match="CZSC frequency unavailable: missing"):
            method("missing")
    for bar in bars[1:650]:
        analysis.update_signals(bar)
        check_reads(analysis)
    assert analysis.last_bi("30分钟") is not None
    assert analysis.structure_state("30分钟") == analysis.structure_state("30分钟", 1)


def test_same_dt_revision_and_restore_keep_the_existing_engine_contract(bars):
    analysis = engine(bars[:99])
    for bar in bars[99:450]:
        analysis.update_signals(bar)
    original = bars[449]
    revised = czsc.RawBar(
        original.symbol,
        original.dt,
        original.freq,
        original.open,
        original.low,
        original.high,
        original.low,
        original.vol,
        original.amount,
        original.id,
    )
    for bar in (revised, revised, original):
        analysis.update_signals(bar)
        check_reads(analysis)
    for restored in (copy.copy(analysis), copy.deepcopy(analysis), pickle.loads(pickle.dumps(analysis))):
        # Existing restore reconstructs from retained BG. Compare each restored
        # engine's original API, not a stronger invented historical promise.
        check_reads(restored)
        restored.update_signals(bars[450])
        check_reads(restored)

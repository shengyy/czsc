"""Offline acceptance probe, using czsc.mock only; never a market-data connector.

Run against the installed wheel with python -I. --talib requires the independent
TA-Lib test oracle installed in the task's isolated environment, not a CZSC
runtime dependency. This script does not implement EMA or signal logic.
"""

import argparse
import json
import time
from collections import Counter

import numpy as np

import czsc
from czsc.mock import generate_symbol_kines


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--talib", action="store_true")
    args = parser.parse_args()
    bars = czsc.format_standard_kline(
        generate_symbol_kines("000001", "30分钟", "20230101", "20250101", seed=42), freq=czsc.Freq.F30
    )
    c = czsc.CZSC(bars[:200], max_bi_num=50, min_bi_len=6)
    stages = Counter()
    calls = []
    errors = []
    for i, bar in enumerate(bars[200:], 200):
        c.update(bar)
        if i % 31:
            continue
        before = time.perf_counter()
        snapshot = czsc.get_structure(c)
        calls.append(time.perf_counter() - before)
        for result in snapshot["results"]:
            if result["status"] in ("confirmed_valid", "forming_retest"):
                stages[f"{result['kind']}:{result['direction']}:{result['status']}"] += 1
        if args.talib:
            import talib

            expected = talib.MACD(np.array([b.close for b in c.bars_raw]), fastperiod=12, slowperiod=26, signalperiod=9)
            for key, values in zip(("dif", "dea", "hist"), expected, strict=True):
                actual = np.array([r[key] if r[key] is not None else np.nan for r in snapshot["macd"]["bars"]])
                np.testing.assert_array_equal(np.isnan(actual), np.isnan(values))
                np.testing.assert_allclose(actual, values, atol=1e-11, rtol=1e-11, equal_nan=True)
                errors.append(float(np.nanmax(np.abs(actual - values))))
    report = {
        "version": czsc.__version__,
        "wheel_package": czsc.__file__,
        "mock_input_bars": len(bars),
        "snapshots": len(calls),
        "retained_bars": len(c.bars_raw),
        "retained_bis": len(c.bi_list),
        "snapshot_p50_ms": float(np.median(calls) * 1000),
        "snapshot_p95_ms": float(np.quantile(calls, 0.95) * 1000),
        "snapshot_json_bytes": len(json.dumps(snapshot, ensure_ascii=False).encode()),
        "stages_seen": dict(stages),
        "talib_max_absolute_error": max(errors) if errors else None,
    }
    if args.talib:
        report["talib_version"] = talib.__version__
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()

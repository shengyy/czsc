"""Offline daily-prefix cost comparison through the installed release wheel.

Run with python -I. Uses only czsc.mock and native update/read, no market access
or application geometry. Each mode gets a fresh analyzer with identical input.
The timed calls include native computation, serde and PyO3 JSON decoding. Byte
counting, hashing, counters, initialization and the final full package are outside
that timer. Alternating run order reduces (but cannot remove) machine noise.
"""

import argparse
import hashlib
import importlib.metadata
import json
import platform
import time
from collections import Counter
from pathlib import Path

import numpy as np

import czsc
from czsc.mock import generate_symbol_kines

STATE_FIELDS = (
    "basis",
    "di",
    "as_of",
    "status",
    "previous_zs",
    "latest_finished_zs",
    "zs_candidate",
    "candidate_stroke",
    "results",
)


def encode(value):
    return json.dumps(value, ensure_ascii=False, allow_nan=False, sort_keys=True, separators=(",", ":")).encode()


def latency(ns):
    ms = np.array(ns) / 1_000_000
    return {"total_ms": float(ms.sum()), "p50_ms": float(np.median(ms)), "p95_ms": float(np.quantile(ms, 0.95))}


def observe(bars, warmup, mode):
    c = czsc.CZSC(bars[:warmup], max_bi_num=50, min_bi_len=6)
    read = czsc.get_structure_state if mode == "light" else czsc.get_structure
    update_ns, read_ns, sizes = [], [], []
    digest, stages = hashlib.sha256(), Counter()
    start_loop = time.perf_counter_ns()
    for bar in bars[warmup:]:
        t0 = time.perf_counter_ns()
        c.update(bar)
        t1 = time.perf_counter_ns()
        view = read(c)
        t2 = time.perf_counter_ns()
        update_ns.append(t1 - t0)
        read_ns.append(t2 - t1)
        sizes.append(len(encode(view)))
        if mode == "light":
            assert set(view) == set(STATE_FIELDS)
        state = {key: view[key] for key in STATE_FIELDS}
        digest.update(encode(state) + b"\n")
        stages.update(
            f"{r['kind']}:{r['direction']}:{r['status']}"
            for r in state["results"]
            if r["status"] in ("confirmed_valid", "forming_retest")
        )
        # Drop outside the timed interval: compare call costs, not deallocation
        # of the previous mode's differently sized returned Python dict.
        del view
    loop_ms = (time.perf_counter_ns() - start_loop) / 1_000_000
    final = czsc.get_structure(c)  # one final full package, not per-prefix work
    assert state == {key: final[key] for key in STATE_FIELDS}
    return {
        "mode": mode,
        "update_calls": len(update_ns),
        "read_calls": len(read_ns),
        "extra_final_full_calls_outside_timer": 1,
        "update": latency(update_ns),
        "read": latency(read_ns),
        "update_plus_read": latency([a + b for a, b in zip(update_ns, read_ns, strict=True)]),
        "loop_wall_ms_including_measurement_bookkeeping": loop_ms,
        "returned_json_bytes": {
            "total": sum(sizes),
            "p50": float(np.median(sizes)),
            "p95": float(np.quantile(sizes, 0.95)),
            "max": max(sizes),
            "last": sizes[-1],
        },
        "all_prefix_states_sha256": digest.hexdigest(),
        "stages_seen": dict(sorted(stages.items())),
        "final_retained_bars": len(c.bars_raw),
        "final_retained_bis": len(c.bi_list),
    }, state


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--warmup", type=int, default=200)
    parser.add_argument("--rounds", type=int, default=2)
    parser.add_argument("--example-out", type=Path)
    args = parser.parse_args()
    df = generate_symbol_kines("000001", "日线", "20100101", "20250101", seed=7)
    bars = czsc.format_standard_kline(df, freq=czsc.Freq.D)
    if not 34 <= args.warmup < len(bars) or args.rounds < 1:
        parser.error("warmup must be >=34 and below input length; rounds must be positive")
    runs = []
    for round_index in range(args.rounds):
        for mode in ("full", "light") if round_index % 2 == 0 else ("light", "full"):
            run, state = observe(bars, args.warmup, mode)
            run["round"] = round_index + 1
            runs.append(run)
    assert len({r["all_prefix_states_sha256"] for r in runs}) == 1, "light/full prefix state mismatch"
    assert len({encode(r["stages_seen"]) for r in runs}) == 1
    if args.example_out:
        args.example_out.write_bytes(encode(state) + b"\n")
    print(
        json.dumps(
            {
                "version": czsc.__version__,
                "wheel_package": czsc.__file__,
                "python": platform.python_version(),
                "platform": platform.platform(),
                "wbt_version": importlib.metadata.version("wbt"),
                "sample": {
                    "source": "czsc.mock.generate_symbol_kines",
                    "symbol": "000001",
                    "freq": "日线",
                    "start": "20100101",
                    "end": "20250101",
                    "seed": 7,
                    "frame_sha256": hashlib.sha256(df.to_json(date_format="iso", orient="split").encode()).hexdigest(),
                },
                "input_bars": len(bars),
                "warmup_bars": args.warmup,
                "prefix_calls_per_run": len(bars) - args.warmup,
                "analyzer": {"max_bi_num": 50, "min_bi_len": 6},
                "runs": runs,
                "timing_scope": "update + read includes native calculation/serde/PyO3 decoding; excludes initialization, "
                "Python JSON byte counting, equality hashing, counters, returned-dict deletion and final full package",
                "bytes_format": "UTF-8 JSON, ensure_ascii=False, compact separators; all prefixes measured",
                "limits": "Single local serial mock daily stream; no 4000-symbol pool, concurrency, application observer/storage, "
                "natural-market or premarket wall-clock acceptance. Light still computes qualification MACD and "
                "canonical references; it removes all-BI power and full raw-series projection, not that evaluation.",
            },
            indent=2,
            ensure_ascii=False,
        )
    )


if __name__ == "__main__":
    main()

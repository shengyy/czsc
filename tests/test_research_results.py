"""The public entry points preserve the native payload and their result classes."""

import pandas as pd

from czsc.mock import generate_symbol_kines
from czsc.models import ReplayResult, ResearchResult
from czsc.research import run_replay, run_research


def test_research_and_replay_result_types_and_tables(tmp_path):
    frame = generate_symbol_kines("RESULT", "5分钟", "20240102", "20240103", seed=42)
    frame["vol"] = frame["vol"].astype(float)
    strategy = {
        "symbol": "RESULT",
        "base_freq": "5分钟",
        "signals_config": [],
        "positions": [
            {
                "name": "inactive",
                "symbol": "RESULT",
                "opens": [],
                "exits": [],
                "interval": 0,
                "timeout": 1,
                "stop_loss": 0.0,
                "T0": False,
            }
        ],
        "market": "A股",
    }
    research = run_research(frame, strategy)
    replay = run_replay(frame, strategy, res_path=tmp_path)
    assert type(research) is ResearchResult
    assert type(replay) is ReplayResult
    assert {k: v for k, v in replay.meta.items() if k != "elapsed_ms"} == {
        k: v for k, v in research.meta.items() if k != "elapsed_ms"
    }
    for table in ("signals", "pairs", "holds"):
        expected = getattr(research, f"{table}_df")()
        pd.testing.assert_frame_equal(getattr(replay, f"{table}_df")(), expected)
        assert getattr(research, f"{table}_path") is None
        pd.testing.assert_frame_equal(pd.read_parquet(getattr(replay, f"{table}_path")), expected)

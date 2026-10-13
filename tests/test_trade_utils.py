"""
author: zengbin93
email: zeng_bin8888@163.com
create_dt: 2023/3/21 16:09
describe: 测试交易工具函数（``czsc.resample_to_daily`` 等）

    ``cal_trade_price`` 已在二阶段清理 PR-B 删除，原 ``test_trade_price`` 测试
    随之移除；本文件保留 ``czsc.resample_to_daily`` 的覆盖。
"""

import pandas as pd
import pytest

import czsc
from czsc.mock import generate_symbol_kines


@pytest.fixture
def snapshots():
    # Three snapshots; the middle one removes A rather than carrying it forward.
    frames = [
        generate_symbol_kines(symbol, "日线", "20240101", "20240112", seed=seed)
        for symbol, seed in [("TEST_A", 42), ("TEST_B", 7)]
    ]
    df = pd.concat(frames, ignore_index=True)[["dt", "symbol", "close", "vol"]]
    df = df[df["dt"].isin(pd.to_datetime(["2024-01-05", "2024-01-09", "2024-01-11"]))]
    df = df[~((df["symbol"] == "TEST_A") & (df["dt"] == pd.Timestamp("2024-01-09")))]
    return df.sort_values(["dt", "symbol"]).reset_index(drop=True)


def assert_daily_rows(result, source, expected):
    """Expected pairs specify the date and original row indices, without resampling."""
    dates = [pd.Timestamp(date) for date, indices in expected for _ in indices]
    indices = [i for _, rows in expected for i in rows]
    assert pd.api.types.is_datetime64_any_dtype(result["dt"])
    assert result["dt"].tolist() == dates
    pd.testing.assert_frame_equal(
        result.drop(columns="dt"),
        source.iloc[indices].drop(columns="dt").reset_index(drop=True),
    )


@pytest.mark.parametrize("unit", [None, "ns", "us", "ms", "s"])
@pytest.mark.parametrize("only_trade_date", [True, False])
@pytest.mark.parametrize("explicit_bounds", [True, False])
def test_resample_to_daily_preserves_dates_and_snapshot_rows(snapshots, unit, only_trade_date, explicit_bounds):
    source = snapshots.copy()
    # Exercise pandas' default string parsing as well as every legal datetime unit.
    source["dt"] = source["dt"].dt.strftime("%Y-%m-%d") if unit is None else source["dt"].astype(f"datetime64[{unit}]")
    original = source.copy(deep=True)
    bounds = {"sdt": "2024-01-03", "edt": "2024-01-12"} if explicit_bounds else {}
    result = czsc.resample_to_daily(source, only_trade_date=only_trade_date, **bounds)
    expected = [("2024-01-05", [0, 1])]
    if not only_trade_date:
        expected += [("2024-01-06", [0, 1]), ("2024-01-07", [0, 1])]
    expected += [("2024-01-08", [0, 1]), ("2024-01-09", [2]), ("2024-01-10", [2]), ("2024-01-11", [3, 4])]
    if explicit_bounds:
        expected += [("2024-01-12", [3, 4])]
    assert_daily_rows(result, original, expected)
    # The existing API parses the input dt column in place, but leaves other values intact.
    pd.testing.assert_frame_equal(source.drop(columns="dt"), original.drop(columns="dt"))
    assert source["dt"].tolist() == snapshots["dt"].tolist()


@pytest.mark.parametrize("finer_source", [True, False])
def test_resample_to_daily_does_not_round_either_date_key(snapshots, finer_source):
    source = snapshots.copy()
    tick = pd.Timedelta(1, "ns")
    if finer_source:
        source["dt"] = source["dt"].astype("datetime64[ns]") + tick
        start = pd.Timestamp("2024-01-05")
        # The first observation is after midnight; Jan 5 cannot use future data.
        expected = [
            ("2024-01-06", [0, 1]),
            ("2024-01-07", [0, 1]),
            ("2024-01-08", [0, 1]),
            ("2024-01-09", [0, 1]),
            ("2024-01-10", [2]),
        ]
    else:
        source["dt"] = source["dt"].astype("datetime64[s]")
        start = pd.Timestamp("2024-01-05") + tick
        expected = [
            ("2024-01-05", [0, 1]),
            ("2024-01-06", [0, 1]),
            ("2024-01-07", [0, 1]),
            ("2024-01-08", [0, 1]),
            ("2024-01-09", [2]),
            ("2024-01-10", [2]),
        ]
        expected = [(pd.Timestamp(date) + tick, indices) for date, indices in expected]
    result = czsc.resample_to_daily(source, sdt=start, edt=expected[-1][0], only_trade_date=False)
    assert_daily_rows(result, source, expected)


@pytest.mark.parametrize("unit", ["ns", "us", "ms", "s"])
def test_resample_to_daily_preserves_timezone(snapshots, unit):
    source = snapshots.copy()
    source["dt"] = source["dt"].dt.tz_localize("Asia/Shanghai").astype(pd.DatetimeTZDtype(unit, "Asia/Shanghai"))
    expected = [
        ("2024-01-05", [0, 1]),
        ("2024-01-08", [0, 1]),
        ("2024-01-09", [2]),
        ("2024-01-10", [2]),
        ("2024-01-11", [3, 4]),
    ]
    expected = [(pd.Timestamp(date, tz="Asia/Shanghai"), indices) for date, indices in expected]
    result = czsc.resample_to_daily(source)
    assert_daily_rows(result, source, expected)

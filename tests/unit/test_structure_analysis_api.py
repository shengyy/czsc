from __future__ import annotations

from datetime import datetime, timedelta

import pytest

from czsc import (
    BI,
    CZSC,
    FX,
    Direction,
    Freq,
    Mark,
    NewBar,
    RawBar,
    check_gap_info,
    create_fake_bis,
    format_standard_kline,
    get_zs_seq,
    is_bis_down,
    is_bis_up,
    is_symmetry_zs,
)
from czsc.mock import generate_symbol_kines


def _fx(dt: datetime, mark: Mark, price: float) -> FX:
    high, low = (price, price - 1) if mark == Mark.G else (price + 1, price)
    return FX("000001", dt, mark, high, low, price, [])


def _bi(dt: datetime, start: float, end: float) -> BI:
    direction, mark_a, mark_b = (Direction.Up, Mark.D, Mark.G) if end > start else (Direction.Down, Mark.G, Mark.D)
    fx_a = _fx(dt, mark_a, start)
    fx_b = _fx(dt + timedelta(minutes=10), mark_b, end)
    bar = NewBar(
        "000001",
        dt,
        Freq.F30,
        (start + end) / 2,
        (start + end) / 2,
        max(start, end),
        min(start, end),
        100,
        1000,
    )
    return BI("000001", direction, fx_a, fx_b, [fx_a, fx_b], [bar])


def test_structure_analysis_functions_are_callable_on_empty_inputs() -> None:
    assert create_fake_bis([]) == []
    assert get_zs_seq([]) == []
    assert not is_symmetry_zs([])
    assert not is_bis_up([])
    assert not is_bis_down([])
    assert check_gap_info([]) == []


def test_czsc_exposes_zs_list_property() -> None:
    assert hasattr(CZSC, "zs_list")


def test_create_fake_bis_rejects_non_alternating_fxs() -> None:
    dt = datetime(2024, 1, 1)
    fxs = [
        FX("000001", dt, Mark.G, 10, 9, 10, []),
        FX("000001", dt + timedelta(minutes=1), Mark.G, 11, 10, 11, []),
    ]
    with pytest.raises(ValueError, match="相邻分型标记必须不同"):
        create_fake_bis(fxs)


def test_create_fake_bis_returns_expected_values_for_valid_fxs() -> None:
    dt = datetime(2024, 1, 1)
    fake_bis = create_fake_bis([_fx(dt, Mark.D, 9), _fx(dt + timedelta(minutes=1), Mark.G, 12)])
    assert len(fake_bis) == 1
    assert fake_bis[0].direction == Direction.Up
    assert (fake_bis[0].high, fake_bis[0].low, fake_bis[0].power) == (12, 9, 3)


def test_zs_symmetry_and_trend_functions_on_non_empty_bis() -> None:
    dt = datetime(2024, 1, 1)
    symmetric = [
        _bi(dt, 12, 9),
        _bi(dt + timedelta(minutes=20), 9, 12),
        _bi(dt + timedelta(minutes=40), 12, 9),
    ]
    zss = get_zs_seq(symmetric)
    assert len(zss) == 1
    assert len(zss[0].bis) == 2  # first BI is explicit entering context
    assert zss[0].sdt == symmetric[1].sdt
    assert is_symmetry_zs(symmetric, th=0.01)

    up = [
        _bi(dt, 9, 12),
        _bi(dt + timedelta(minutes=20), 12, 10),
        _bi(dt + timedelta(minutes=40), 10, 14),
    ]
    assert is_bis_up(up)
    assert not is_bis_down(up)

    down = [
        _bi(dt, 14, 10),
        _bi(dt + timedelta(minutes=20), 10, 12),
        _bi(dt + timedelta(minutes=40), 12, 8),
    ]
    assert is_bis_down(down)
    assert not is_bis_up(down)


def test_czsc_zs_list_matches_finished_bis_analysis() -> None:
    df = generate_symbol_kines("000001", "30分钟", sdt="20240101", edt="20240201", seed=42)
    c = CZSC(format_standard_kline(df, Freq.F30))
    expected = get_zs_seq(c.finished_bis)
    assert len(c.zs_list) == len(expected)
    assert [len(zs.bis) for zs in c.zs_list] == [len(zs.bis) for zs in expected]
    assert [(z.sdt, z.edt, z.zd, z.zg, z.dd, z.gg) for z in c.zs_list] == [
        (z.sdt, z.edt, z.zd, z.zg, z.dd, z.gg) for z in expected
    ]


@pytest.mark.parametrize("mirror", [False, True])
@pytest.mark.parametrize(
    "prices, lengths, bounds",
    [
        ([20, 14, 18, 15, 19, 10, 16, 11, 14], [4, 3], [(15, 18), (11, 14)]),
        ([24, 12, 20, 14, 15, 14.5, 17, 16, 16.5, 11], [5, 3], [(14, 15), (16, 16.5)]),
    ],
)
def test_canonical_core_boundary_examples_through_python(prices, lengths, bounds, mirror) -> None:
    # Shared, direction-ordered nonzero-width FX intervals exercise the exact
    # reported geometric examples through the public Rust binding.
    prices = [60 - p if mirror else p for p in prices]
    fxs = []
    for i, p in enumerate(prices):
        top = p > prices[1] if i == 0 else p > prices[i - 1]
        fxs.append(
            FX(
                "000001",
                datetime(2024, 1, 1) + timedelta(minutes=10 * i),
                Mark.G if top else Mark.D,
                p if top else p + 0.1,
                p - 0.1 if top else p,
                p,
                [],
            )
        )
    bis = [
        BI("000001", Direction.Up if b.fx > a.fx else Direction.Down, a, b, [], [])
        for a, b in zip(fxs[:-1], fxs[1:], strict=True)
    ]
    centres = get_zs_seq(bis)
    assert [len(z.bis) for z in centres] == lengths
    assert [(z.zd, z.zg) for z in centres] == [(60 - zg, 60 - zd) if mirror else (zd, zg) for zd, zg in bounds]
    assert all(z.is_valid() for z in centres)
    assert centres[0].sdt == bis[1].sdt  # entering BI is not part of its core
    members = [b.sdt for z in centres for b in z.bis]
    assert len(members) == len(set(members))


def test_check_gap_info_returns_compatible_dicts() -> None:
    dt = datetime(2024, 1, 1)
    bars = [
        RawBar("000001", dt, Freq.F30, 9.5, 9.5, 10, 9, 100, 1000, 1),
        RawBar("000001", dt + timedelta(minutes=30), Freq.F30, 11.5, 11.5, 12, 11, 100, 1000, 2),
        RawBar("000001", dt + timedelta(minutes=60), Freq.F30, 10, 10, 11, 9.5, 100, 1000, 3),
    ]
    gaps = check_gap_info(bars)
    assert len(gaps) == 1
    assert gaps[0]["kind"] == "向上缺口"
    assert gaps[0]["cover"] == "已补"
    assert gaps[0]["sdt"].to_pydatetime() == dt
    assert gaps[0]["edt"].to_pydatetime() == dt + timedelta(minutes=30)
    assert gaps[0]["high"] == 11.0
    assert gaps[0]["low"] == 10.0
    assert gaps[0]["delta"] == 0.1


def test_check_gap_info_handles_down_gap_and_no_gap() -> None:
    dt = datetime(2024, 1, 1)
    down_gap = [
        RawBar("000001", dt, Freq.F30, 11.5, 11.5, 12, 11, 100, 1000, 1),
        RawBar("000001", dt + timedelta(minutes=30), Freq.F30, 9.5, 9.5, 10, 9, 100, 1000, 2),
        RawBar("000001", dt + timedelta(minutes=60), Freq.F30, 10.5, 10.5, 11.5, 9.5, 100, 1000, 3),
    ]
    gaps = check_gap_info(down_gap)
    assert len(gaps) == 1
    assert gaps[0]["kind"] == "向下缺口"
    assert gaps[0]["cover"] == "已补"

    no_gap = [
        RawBar("000001", dt, Freq.F30, 9.5, 9.5, 10, 9, 100, 1000, 1),
        RawBar("000001", dt + timedelta(minutes=30), Freq.F30, 10, 10, 10.5, 9.5, 100, 1000, 2),
    ]
    assert check_gap_info(no_gap) == []

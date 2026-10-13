"""``czsc.utils`` 通用工具函数单元测试。

模块作者：
    zengbin93 (zeng_bin8888@163.com)
"""

import pytest

from czsc.mock import generate_symbol_kines
from czsc.utils import freqs_sorted, print_df_sample


@pytest.mark.parametrize("n", [None, 0, 2, 100])
def test_print_df_sample(n, capsys):
    frame = generate_symbol_kines("SAMPLE", "日线", "20240101", "20240115", seed=42)
    # Text columns let the assertion check exact rows without depending on numeric formatting.
    frame = frame[["dt", "symbol"]].astype(str)
    assert len(frame) > 5
    if n is None:
        print_df_sample(frame)
        count = 5
    else:
        print_df_sample(frame, n)
        count = min(n, len(frame))
    output = capsys.readouterr().out
    rows = [line.split() for line in output.splitlines() if "SAMPLE" in line]
    assert rows == [[str(dt), "SAMPLE"] for dt in frame["dt"].iloc[:count]]
    assert "dt" in output and "symbol" in output
    assert "===" in output


def test_freqs_sorted_basic():
    """验证 ``freqs_sorted`` 按缠论惯用顺序对周期字符串去重并排序。

    测试场景：
        - 输入打乱顺序的周期列表，包含重复项
        - 应去重后按从高频到低频顺序输出

    关键断言：
        - ``'5分钟'`` 排在 ``'30分钟'`` 之前
        - ``'30分钟'`` 排在 ``'日线'`` 之前
        - 重复项被去重
    """
    result = freqs_sorted(["30分钟", "5分钟", "日线", "5分钟"])
    assert result == ["5分钟", "30分钟", "日线"]


def test_freqs_sorted_unknown_freq_to_tail():
    """验证未登记的周期被排到末尾，并按字典序作次级排序。"""
    result = freqs_sorted(["日线", "未知A", "5分钟", "未知B"])
    assert result[:2] == ["5分钟", "日线"]
    # 未登记的两个周期排在末尾，按字典序
    assert set(result[2:]) == {"未知A", "未知B"}

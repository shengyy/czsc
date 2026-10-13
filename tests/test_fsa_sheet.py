"""Sheet selection is tested without opening a Feishu connection."""

from unittest.mock import Mock

import pytest

from czsc import fsa


@pytest.mark.parametrize("sheet_id", [None, "chosen"])
def test_read_feishu_sheet_selection(monkeypatch, sheet_id):
    sheets = Mock()
    sheets.get_sheets.return_value = {"data": {"sheets": [{"sheet_id": "first"}]}}
    factory = Mock(return_value=sheets)
    monkeypatch.setattr(fsa, "SpreadSheets", factory)
    result = fsa.read_feishu_sheet("document", sheet_id, feishu_app_id="test", feishu_app_secret="test")
    factory.assert_called_once_with(app_id="test", app_secret="test")
    sheets.read_table.assert_called_once_with("document", sheet_id or "first")
    assert result is sheets.read_table.return_value
    if sheet_id is None:
        sheets.get_sheets.assert_called_once_with("document")
    else:
        sheets.get_sheets.assert_not_called()

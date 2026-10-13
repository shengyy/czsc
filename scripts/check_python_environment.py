"""Check the selected interpreter and locked pandas before building or testing."""

import argparse
import sys
from importlib.metadata import version
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--interpreter-only", action="store_true", help="Check before dependencies are installed")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    expected_python = (root / ".python-version").read_text().strip()
    actual = f"{sys.version_info.major}.{sys.version_info.minor}"
    if actual != expected_python:
        raise SystemExit(f"Expected Python {expected_python} from .python-version, got {actual}: {sys.executable}")
    if args.interpreter_only:
        print(f"Python {actual}: {sys.executable}")
        return

    import tomllib

    lock = tomllib.loads((root / "uv.lock").read_text())
    (expected_pandas,) = [p["version"] for p in lock["package"] if p["name"] == "pandas"]
    if version("pandas") != expected_pandas:
        raise SystemExit(f"Expected pandas {expected_pandas} from uv.lock, got {version('pandas')}")

    import pandas as pd

    # Import alone does not exercise pandas' datetime conversion binaries.
    expected = pd.date_range("2024-01-01", periods=2)
    assert pd.to_datetime(["2024-01-01", "2024-01-02"]).equals(expected)
    print(f"Python {actual}: {sys.executable}; pandas {version('pandas')}; numpy {version('numpy')}; datetime OK")


if __name__ == "__main__":
    main()

"""Check the selected interpreter and locked pandas before building or testing."""

import argparse
import re
import sys
import tomllib
from importlib.metadata import version
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--interpreter-only", action="store_true", help="Check before dependencies are installed")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    required = tomllib.loads((root / "pyproject.toml").read_text())["project"]["requires-python"]
    minimum = re.fullmatch(r">=(\d+)\.(\d+)", required)
    if minimum is None:
        raise SystemExit("requires-python must declare a >=major.minor lower bound")
    actual = f"{sys.version_info.major}.{sys.version_info.minor}"
    if sys.version_info[:2] < tuple(map(int, minimum.groups())):
        raise SystemExit(f"Python {actual} does not satisfy {required} from pyproject.toml: {sys.executable}")
    if args.interpreter_only:
        print(f"Python {actual}: {sys.executable}")
        return

    from packaging.markers import Marker

    lock = tomllib.loads((root / "uv.lock").read_text())
    expected_pandas = {
        p["version"]
        for p in lock["package"]
        if p["name"] == "pandas"
        and (not p.get("resolution-markers") or any(Marker(m).evaluate() for m in p["resolution-markers"]))
    }
    if version("pandas") not in expected_pandas:
        expected = ", ".join(sorted(expected_pandas)) or "a matching interpreter branch"
        raise SystemExit(f"Expected pandas {expected} from uv.lock, got {version('pandas')}")

    import pandas as pd

    # Import alone does not exercise pandas' datetime conversion binaries.
    expected = pd.date_range("2024-01-01", periods=2)
    assert pd.to_datetime(["2024-01-01", "2024-01-02"]).equals(expected)
    print(f"Python {actual}: {sys.executable}; pandas {version('pandas')}; numpy {version('numpy')}; datetime OK")


if __name__ == "__main__":
    main()

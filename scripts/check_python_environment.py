"""Fail CI early on interpreter drift or incompatible pandas datetime binaries."""

import argparse
import sys
from importlib.metadata import version

import pandas as pd

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--expected-python", required=True, help="CI matrix major.minor, e.g. 3.14")
parser.add_argument("--expected-pandas", help="Exact pandas version selected by the development lock")
args = parser.parse_args()
actual = f"{sys.version_info.major}.{sys.version_info.minor}"
if actual != args.expected_python:
    raise SystemExit(f"Expected Python {args.expected_python}, got {actual}: {sys.executable}")
if args.expected_pandas and version("pandas") != args.expected_pandas:
    raise SystemExit(f"Expected pandas {args.expected_pandas}, got {version('pandas')}")
if sys.version_info >= (3, 14) and tuple(int(x) for x in version("pandas").split(".")[:3]) < (2, 3, 3):
    raise SystemExit("Python >= 3.14 requires pandas >= 2.3.3")
# Import alone did not expose the old pandas 2.3.2 / Python 3.14 SIGSEGV.
expected = pd.date_range("2024-01-01", periods=2)
assert pd.to_datetime(["2024-01-01", "2024-01-02"]).equals(expected)
print(f"Python {actual}: {sys.executable}; pandas {version('pandas')}; numpy {version('numpy')}; datetime OK")

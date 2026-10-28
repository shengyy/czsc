"""Exercise build.rs itself without recompiling the native dependency graph."""

import os
import subprocess
import sys
import tomllib
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
pytestmark = pytest.mark.slow


@pytest.fixture
def build_guard(tmp_path):
    manifest = tmp_path / "crates/czsc-python"
    manifest.mkdir(parents=True)
    (tmp_path / ".python-version").write_text((ROOT / ".python-version").read_text())
    required = tomllib.loads((ROOT / "pyproject.toml").read_text())["project"]["requires-python"]
    (tmp_path / "pyproject.toml").write_text(f'[project]\ndynamic = ["version"]\nrequires-python = "{required}"\n')
    executable = tmp_path / ("build-check.exe" if sys.platform == "win32" else "build-check")
    env = {**os.environ, "CARGO_MANIFEST_DIR": str(manifest)}
    subprocess.run(
        ["rustc", "--edition=2024", str(ROOT / "crates/czsc-python/build.rs"), "-o", str(executable)],
        env=env,
        check=True,
        capture_output=True,
        text=True,
    )
    return executable


def run_guard(executable, interpreter):
    env = {**os.environ, "PYO3_CONFIG_FILE": "target-abi-config"}
    env.pop("PYO3_PYTHON", None)
    if interpreter:
        env["PYO3_PYTHON"] = interpreter
    return subprocess.run([str(executable)], env=env, capture_output=True, text=True)


def test_build_accepts_selected_host_with_target_abi_config(build_guard):
    result = run_guard(build_guard, sys.executable)
    assert result.returncode == 0, result.stderr


def test_build_does_not_fallback_or_bypass_host_selection(build_guard):
    result = run_guard(build_guard, None)
    assert result.returncode != 0
    assert "set PYO3_PYTHON" in result.stderr


def test_build_rejects_unsupported_interpreter(build_guard):
    (build_guard.parent / "pyproject.toml").write_text('[project]\ndynamic = ["version"]\nrequires-python = ">=99.0"\n')
    result = run_guard(build_guard, sys.executable)
    assert result.returncode != 0
    assert "does not satisfy >=99.0" in result.stderr


def test_build_allows_explicit_host_other_than_default_development_version(build_guard):
    (build_guard.parent / ".python-version").write_text("0.0\n")
    result = run_guard(build_guard, sys.executable)
    assert result.returncode == 0, result.stderr

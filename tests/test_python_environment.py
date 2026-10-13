"""The environment guard consumes the checkout declarations, not CI version literals."""

import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

import pytest
from packaging.specifiers import SpecifierSet

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/check_python_environment.py"


def run_guard(root, *args):
    return subprocess.run(
        [sys.executable, str(root / "scripts/check_python_environment.py"), *args], capture_output=True, text=True
    )


@pytest.fixture
def checkout(tmp_path):
    (tmp_path / "scripts").mkdir()
    shutil.copy(SCRIPT, tmp_path / "scripts")
    for name in (".python-version", "uv.lock"):
        shutil.copy(ROOT / name, tmp_path / name)
    return tmp_path


def test_environment_matches_checkout():
    result = run_guard(ROOT)
    assert result.returncode == 0, result.stderr
    assert "datetime OK" in result.stdout


def test_wrong_interpreter_is_rejected_before_dependencies(checkout):
    (checkout / ".python-version").write_text("0.0\n")
    (checkout / "uv.lock").unlink()
    result = run_guard(checkout, "--interpreter-only")
    assert result.returncode != 0
    assert "Expected Python 0.0" in result.stderr


def test_dependency_drift_is_rejected(checkout):
    (checkout / "uv.lock").write_text('[[package]]\nname = "pandas"\nversion = "0.0.0"\n')
    result = run_guard(checkout)
    assert result.returncode != 0
    assert "Expected pandas 0.0.0 from uv.lock" in result.stderr


def test_interpreter_selection_matches_public_support():
    project = tomllib.loads((ROOT / "pyproject.toml").read_text())["project"]
    selected = (ROOT / ".python-version").read_text().strip()
    assert selected in SpecifierSet(project["requires-python"])
    assert [x for x in project["classifiers"] if x.startswith("Programming Language :: Python ::")] == [
        f"Programming Language :: Python :: {selected}"
    ]


def test_runtime_sync_retains_locked_build_frontend():
    result = subprocess.run(
        ["uv", "sync", "--locked", "--no-dev", "--dry-run"], cwd=ROOT, capture_output=True, text=True
    )
    assert result.returncode == 0, result.stderr


def test_runtime_export_excludes_build_and_audit_tools():
    result = subprocess.run(
        ["uv", "export", "--locked", "--no-default-groups", "--no-emit-project"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stderr
    requirements = result.stdout.splitlines()
    assert any(line.startswith("pandas==") for line in requirements)
    assert not any(line.startswith(("maturin==", "safety==", "ruff==", "pytest==")) for line in requirements)

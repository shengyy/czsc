"""安装产物烟雾测试：maturin wheel 安装与原生扩展可用性验证。

本测试套件用于在最小可运行范围内验证 czsc 的 Rust/Python 混合架构构建产物
（wheel）是否被正确打包，并且能够在干净的虚拟环境中安装并通过基本的导入
冒烟测试。

业务背景：
    项目通过 ``maturin build --release`` 构建跨平台二进制 wheel
    （manylinux/macOS/Windows），其中包含 Rust 编译产物 ``czsc._native``
    动态库。安装该 wheel 后，必须满足：

    1. ``import czsc`` 成功；
    2. ``import czsc._native`` 成功且 ``__file__`` 指向编译产物（.so/.pyd/.dylib）；
    3. 不再需要任何额外的 ``rs_czsc`` 包。

测试覆盖：
    - ``pyproject.toml`` 已使用 ``maturin`` 作为构建后端；
    - 当前安装环境中可成功导入 ``czsc._native``；
    - ``dist/`` 下的 wheel 在干净 venv 中可安装且 ``czsc.CZSC.__module__`` 指向 czsc 命名空间。
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

# 仓库根目录（基于本测试文件位置反推），以及构建产物所在目录
REPO_ROOT = Path(__file__).resolve().parents[2]
DIST_DIR = REPO_ROOT / "dist"


def test_pyproject_uses_maturin_backend() -> None:
    """验证 pyproject.toml 使用 maturin 作为构建后端。

    测试目标：
        确保项目构建系统已切换为 maturin（PyO3 推荐的 Python 扩展打包工具），
        以便能够产出包含 Rust 原生扩展的 wheel。

    关键断言：
        pyproject.toml 文件中包含 ``maturin`` 字符串，意味着 ``[build-system]``
        节点声明了 ``requires=['maturin>=...']`` 与 ``build-backend='maturin'``。
    """
    pyproject = REPO_ROOT / "pyproject.toml"
    if not pyproject.is_file():
        pytest.fail(f"未找到 pyproject.toml，路径：{pyproject}")
    text = pyproject.read_text(encoding="utf-8")
    assert "maturin" in text, (
        "pyproject.toml 必须声明 [build-system] requires=['maturin>=...'] 以及 build-backend='maturin'"
    )


@pytest.mark.slow
def test_native_extension_present_in_install() -> None:
    """验证当前安装环境中已包含编译后的 czsc._native 扩展。

    测试场景：
        通过子进程方式调用 Python 解释器尝试 ``import czsc._native``，
        并打印其 ``__file__`` 属性。

    关键断言：
        - 子进程退出码必须为 0（导入成功）；
        - ``czsc._native.__file__`` 必须以 ``.so`` / ``.pyd`` / ``.dylib`` 结尾，
          表明它是编译生成的二进制扩展，而不是普通 Python 模块。
    """
    proc = subprocess.run(
        [sys.executable, "-c", "import czsc._native; print(czsc._native.__file__)"],
        capture_output=True,
        text=True,
    )
    assert proc.returncode == 0, (
        "在当前安装环境中 `import czsc._native` 必须成功 "
        f"(stderr: {proc.stderr.strip()!r})；"
        "构建流程必须保证 maturin 正确打包了 Rust 扩展"
    )
    out = proc.stdout.strip()
    assert out.endswith((".so", ".pyd", ".dylib")), f"czsc._native.__file__ 应指向编译扩展，实际为 {out!r}"


def test_wheel_install_in_clean_venv(tmp_path: Path) -> None:
    """Install locked runtime dependencies and the wheel, then import outside the checkout."""
    # 选取 dist/ 下的所有 czsc 安装包，并以排序方式取最新版本
    wheels = sorted(DIST_DIR.glob("czsc-*.whl")) if DIST_DIR.is_dir() else []
    if not wheels:
        # 常规 CI 的 test job 只跑 `maturin develop`，不产 wheel；
        # wheel 安装验证由 python-publish.yml 的 smoke-test job 在发布流程
        # 里专门跑。所以本地/常规 CI 没 wheel 时跳过，不算失败。
        pytest.skip(f"在 {DIST_DIR} 下找不到 wheel；如需运行本测试，请先 `maturin build --release`")

    venv = tmp_path / "venv"
    subprocess.run([sys.executable, "-m", "venv", str(venv)], check=True)
    py = venv / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")
    requirements = tmp_path / "runtime.txt"
    subprocess.run(
        ["uv", "export", "--locked", "--no-default-groups", "--no-emit-project", "--output-file", str(requirements)],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    subprocess.run(["uv", "pip", "sync", "--python", str(py), str(requirements)], check=True)
    subprocess.run(["uv", "pip", "install", "--python", str(py), "--no-deps", str(wheels[-1])], check=True)
    subprocess.run([str(py), str(REPO_ROOT / "scripts/check_python_environment.py")], check=True)
    smoke = subprocess.run(
        [
            str(py),
            "-c",
            (
                "import sys, czsc, czsc._native; from pathlib import Path; "
                "assert Path(czsc._native.__file__).is_relative_to(Path(sys.prefix)); "
                "print(czsc.CZSC.__module__)"
            ),
        ],
        cwd=tmp_path,
        capture_output=True,
        text=True,
    )
    assert smoke.returncode == 0, smoke.stderr
    assert "czsc" in smoke.stdout

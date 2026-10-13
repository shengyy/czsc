//! czsc-python 的 build script。
//!
//! 职责：
//!
//! 1. **校验构建解释器**：读取仓库 `.python-version`，要求调用方显式传入
//!    `PYO3_PYTHON`（maturin 自动传入所选解释器；直接 cargo 调用需自行设置）。
//!    不从 PATH 猜测或回退。构建解释器与 abi3 最小 ABI 是不同约束。
//!
//! 2. **macOS cdylib 链接参数**：cdylib 需要 `-undefined dynamic_lookup`，
//!    这样 Python 符号才能在运行时由宿主解释器解析。PyO3 的
//!    `extension-module` feature 一般会自动加上，但当我们直接用
//!    `cargo build --workspace`（不走 maturin）构建时，需要显式声明，
//!    以便 workspace layout test 保持 GREEN。
//!
//! 3. **校验版本号同步**（PR-5）：Rust workspace 与 Python wheel 的版本号必须
//!    锁死同步。`pyproject.toml` 通过 `dynamic = ["version"]` 由 maturin 注入
//!    Cargo workspace 的 `version`，所以这里只需校验 pyproject.toml 仍然走
//!    dynamic 路径，避免有人误把 `version = "x.y.z"` 写进 [project]
//!    导致 Python 与 Rust 出现版本漂移。
//!
//! 任何一步检查失败都会 `panic!`，并附带具体的修复建议。

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=PYO3_PYTHON");
    println!("cargo:rerun-if-env-changed=PYO3_CONFIG_FILE");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");

    check_python_version();
    check_version_lockstep();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg-cdylib=-undefined");
        println!("cargo:rustc-link-arg-cdylib=dynamic_lookup");
    }
}

/// 校验 `pyproject.toml` 的 [project] 段使用 `dynamic = ["version"]`。
///
/// 这是 PR-5 提出的"crates.io 与 PyPI 版本锁死"机制的工程实现：
/// - Cargo workspace 的 `[workspace.package].version` 是唯一版本源；
/// - maturin 在打 wheel 时通过 dynamic 注入该值到 Python metadata。
///
/// 若有人后续把版本直接硬编码到 pyproject.toml 的 `version = "..."`，
/// 我们就在编译期立刻 panic，避免 Python / Rust 两侧版本漂移。
fn check_version_lockstep() {
    // 注意：file 路径相对于 cargo 调用目录（workspace 根），
    // 这里手动定位 workspace 根：CARGO_MANIFEST_DIR 是 crates/czsc-python。
    let manifest_dir: std::path::PathBuf = env!("CARGO_MANIFEST_DIR").into();
    let pyproject = manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace 根
        .map(|p| p.join("pyproject.toml"));
    let Some(pyproject_path) = pyproject else {
        // 无法定位 workspace 根，跳过校验（极端构建环境的兜底）
        return;
    };
    println!("cargo:rerun-if-changed={}", pyproject_path.display());

    let content = match std::fs::read_to_string(&pyproject_path) {
        Ok(s) => s,
        Err(_) => return, // 文件不存在或读不到，跳过
    };

    // 极简解析：进入 [project] 段后查找 `version = ` 或 `dynamic = `
    let mut in_project = false;
    let mut has_dynamic_version = false;
    let mut has_hardcoded_version = false;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_project = line == "[project]";
            continue;
        }
        if !in_project {
            continue;
        }
        // 把所有空白都剔掉再匹配，规避 `dynamic = ["version"]` 与 `dynamic=["version"]`
        let no_space: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        if no_space.starts_with("version=\"") {
            has_hardcoded_version = true;
        }
        if no_space.starts_with("dynamic=") && no_space.contains("\"version\"") {
            has_dynamic_version = true;
        }
    }

    if has_hardcoded_version {
        panic!(
            "czsc-python build.rs: pyproject.toml 的 [project] 段中检测到硬编码的 \
             `version = \"...\"`，违反 PR-5 的版本锁死约定。\n\
             请改为 `dynamic = [\"version\"]`，由 maturin 从 Cargo workspace 注入。"
        );
    }
    if !has_dynamic_version {
        panic!(
            "czsc-python build.rs: pyproject.toml 的 [project] 段未声明 \
             `dynamic = [\"version\"]`，无法从 Cargo workspace 注入版本号。\n\
             请加入 `dynamic = [\"version\"]`（PR-5 版本锁死要求）。"
        );
    }
}

/// 校验显式选择的构建解释器与 `.python-version` 一致。
/// `PYO3_CONFIG_FILE` 描述目标 ABI，不能替代或绕过宿主解释器选择。
fn check_python_version() {
    let version_file =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.python-version");
    println!("cargo:rerun-if-changed={}", version_file.display());
    let expected = std::fs::read_to_string(&version_file)
        .expect("czsc-python: missing .python-version in the source distribution");
    let expected = expected.trim();
    let interpreter = std::env::var("PYO3_PYTHON").expect(
        "czsc-python: set PYO3_PYTHON to the project interpreter; use \
         PYO3_PYTHON=$(uv run --no-sync python -c 'import sys; print(sys.executable)') cargo ...",
    );
    let output = Command::new(&interpreter)
        .args([
            "-c",
            "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')",
        ])
        .output()
        .unwrap_or_else(|err| panic!("czsc-python: cannot run PYO3_PYTHON={interpreter}: {err}"));
    assert!(
        output.status.success(),
        "czsc-python: PYO3_PYTHON={interpreter} failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    let actual = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        actual.trim(),
        expected,
        "czsc-python: PYO3_PYTHON={interpreter} must match .python-version",
    );
}

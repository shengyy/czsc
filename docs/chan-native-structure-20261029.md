# Native BI-proxy structure contract (2026-10-29)


## 本机验收摘要

- 本批只施工 CZSC 上游；canonical 几何 owner 的 `analyze/utils.rs` 与 `e864cfb` 完全相同（SHA-256 `cbae6d14ef9ba9ae9b79acb56f3b82e0721b73049ab20720ef618adc30ae3af5`）。
- 含轻量补充的最终回归：Rust facade/core/signals/trader 250 项通过；安装优化 wheel 后的全部 `tests/unit` 284 项通过。原批 249/283 项保留通过。旧基线的 2 项预期 RED 分别隔离经验笔数门与 MA 唯一阻断；补充 API 在旧 wheel 上另有 1 项预期 RED（接口不存在），最终均 GREEN。
- 独立 TA-Lib 0.8.3：7,320 根 mock / 230 个快照，预热 mask 一致，最大误差 `8.53e-14`。
- 原批 30 分钟 mock 的完整 snapshot（含序列化）抽样 P50 约 2.56 ms / P95 约 3.53 ms，末次 JSON 147,137 bytes。补充已实测日线逐 K：light `update + read` P50 1.03–1.04 ms / P95 1.26–1.28 ms，full P50 1.91–1.97 ms / P95 2.21–2.36 ms；每轮累计 JSON 减少约 94.3%。样本、调用数、字节格式和局限见下方测量表，不代表全市场预算已过。
- 构建：`target/wheels-release/czsc-1.0.1+ashare.20261029-cp310-abi3-macosx_11_0_arm64.whl`。独立环境：本树 `.venv`、`target/chan-native`；下游按最终 Git commit 锁定，不能采用本地路径依赖。
- 完整样例：`target/acceptance/mock-structure-seed7-prefix982.json`；该 mock 固定 `seed=7, min_bi_len=6, max_bi_num=50`，展示 `second_like/Down/confirmed_valid`、有限 B2 父链、新核 `[87.76, 91.23]` 和当前回试。`tests/__init__.py` 的默认笔长为 7，冻结样例显式固定 6，避免隐藏环境改变材料。
- 原批证据：`target/acceptance/{rust-tests.log,python-wheel-tests.log,expected-red-baseline.log,native-oracle-release.json,stub-gen.log,rustfmt.log,lint.log}`。无范围外顺手修复；缓存精度和旧引用解码是本批一致性所需修复。未运行真实 QMT、付费模型回放或自然行情全池验收。
- 轻量补充证据另存 `target/acceptance/light-*`，保留原批日志/样例；`light-rust-tests.log`、`light-python-wheel-tests.log`、`light-daily-prefix-benchmark.json` 对应最终回归/测量，`light-before-prefix-digest.json` 和 `light-frozen-full-parity.json` 证明补充前后同源状态与冻结全量样例一致。


## Scope and ownership

This change implements the approved upstream batch on `e864cfb`. `czsc-core::analyze::utils::get_zs_seq` retains its canonical geometry and membership rules. `czsc-signals::utils::bs` owns auxiliary qualification; public CXT values, `signal_references`, and `get_structure` all call that evaluator. Python only re-exports the native function and serializes the Rust snapshot through PyO3. No application, production configuration, account, broker, or shared environment is changed.

`basis="bi_proxy"` is explicit. These are current BI-level auxiliary relationships, not proof of recursive Chan trends, strict first-buy divergence, or trading readiness. `confirmed_valid` means the required native BIs currently exist and the observed raw path holds; they can still be revised. `forming_retest` is research eligibility only. First points never use that stage.

The reviewed signal entry points are:

| Public name (unchanged) | Implementation |
| --- | --- |
| `cxt_first_buy_V221126`, `cxt_first_sell_V221126` | Same canonical entry/core/departure, new extreme, direction/endpoint guards; native same-direction MACD area weakening |
| `cxt_second_bs_V230320` | Specific first-point anchor, immediately adjacent rebound, first actual retest, strict project anchor subset |
| `cxt_third_buy_V230228`, `cxt_third_bs_V230318` | Latest valid canonical finished-membership core only, native departure, its first actual retest, entire raw path strictly outside ZG/ZD |

The old signal keys, including `SMA#21`/`SMA#34`, are preserved. Their `ma_type`/`timeperiod` arguments are now key compatibility fields, not eligibility filters. No arbitrary total-BI minimum replaces the removed 12/7-BI gates. Other historical strategy versions, e.g. `V230319`/`V240524`, retain their separate legacy semantics and are not inputs to this contract.

## Public API

```python
import czsc
facts = czsc.get_structure(c, di=1)  # c is an existing native CZSC
```

```rust
let facts = czsc::get_structure(&c, 1);
// Same typed implementation is available at czsc::signals::structure::get_structure.
```

`get_structure` is new because an analyzer alone has no signal-cache accessor, while the required snapshot combines core geometry with signal-owned MACD and stages. Putting it on the signals side avoids a core→signals dependency or duplicate Python logic. It accepts an existing analyzer; it does not fetch market data or create a second analyzer. The signal cache and this public read call the same numerical implementation on the exact same retained input.

The returned Python object is a detached `dict`; Rust returns `StructureSnapshot`, serializable with serde. Dates are native UTC-tagged ISO values (the same convention as existing `signal_references`); no timezone conversion occurs here. The caller must preserve its existing market-time coordinate convention.

For per-bar first-known observation use `czsc.get_structure_state(c, di=1)` / Rust `czsc::get_structure_state(&c, 1) -> StructureState`. Both views call the same `structure.rs::evaluate_structure`; only `get_structure` subsequently constructs the full numerical projection. No signal qualification, reference semantics or existing full fields changed in this supplement.

The light view contains exactly `basis, di, as_of, status, previous_zs, latest_finished_zs, zs_candidate, candidate_stroke, results`. Every nested field equals the corresponding full field, including rejected operands, missing reasons and the MACD comparison required to qualify first points/parents. It omits `bis`, `last_signal_bi`, `ubi_newbar_count`, `zs_relation`, `candidate_power`, `macd`. The light path neither computes all-BI/candidate power nor constructs/serializes the per-bar `MacdFacts` projection. Qualification still computes the current retained-input MACD and the needed comparison operands. There is no approximate cache, extra state store, event engine or durable identity.

```python
for bar in received_bars:
    c.update(bar)
    state = czsc.get_structure_state(c)
    # Pass state to the application's existing prefix observer.
facts = czsc.get_structure(c)  # once, when the final numerical package is needed
```

The separate typed view keeps the original Rust `StructureSnapshot` fields and Python `get_structure` shape intact. Python is still a direct native re-export with only the shared serde-to-dict binding. No version bump is made for this same unpublished batch; downstream must pin the final commit, not the reused local wheel filename/version alone.

| Field | Meaning |
| --- | --- |
| `basis`, `di`, `as_of`, `status` | `bi_proxy`, current-list offset, latest received raw cutoff, `ready` or `missing_material` |
| `bis`, `last_signal_bi` | Selected current BI prefix and its last member; natural start/end, direction, high/low, endpoint prices, membership flag, original price amplitude/NewBar count and interval power |
| `previous_zs`, `latest_finished_zs` | Last two valid canonical cores from `finished_bis[:end]`; original boundaries, member `dd/gg`, all members and roles |
| `zs_candidate` | Canonical tail of one/two members only; `valid=false`. Not a mature core |
| `zs_relation` | `basis=bi_proxy`, separate `core` and `outer`: `above`, `below`, `touch`, `overlap`; null without two valid cores |
| `candidate_stroke` | Native NewBar-derived moving segment, described below; never a BI |
| `candidate_power` | `(anchor_at, as_of]` raw volume/amount/MACD values; numerical readiness does not complete a stroke |
| `macd` | Full retained per-bar native sequence, parameters, scale, coverage and truncation flag |
| `results` | First/second/third/second-like auxiliary results in each direction; status, reason and selected role operands |

`bis[].finished` remains for reference compatibility; `included_in_finished_bis` names its membership meaning explicitly. `ubi_newbar_count` explains the existing five-NewBar membership rule. Neither means irrevocable endpoint confirmation. No `end_confirmed`, recursive validity, expansion, or two-pivot-trend boolean is added.

`CentreReference` contains `sdt, edt, zd, zg, zz, dd, gg, bi_count, valid, members, entry, departure, retest`. Membership geometry is exactly the native core. Role references are actual BIs, and may lie outside the core's members: in particular the last native departure may not yet belong to the finished prefix. A crossing leg/adjacent retest in raw structural context is not by itself a qualifying buy/sell result. Missing roles are null.

### Stages and operand identity

Each `results[]` item has `kind` (`first`, `second`, `third`, `second_like`), `direction` (`Down` buy side, `Up` sell side), `status`, `reason`, and `reference`. Eligible statuses are **only** `confirmed_valid` and `forming_retest`. `no_match` includes missing structural relationships with a reason; `unavailable` is indeterminate numerical/coverage material. A rejected evaluation can retain its operands for diagnosis; the caller must not treat a non-null reference as eligibility.

A result reference includes:

```text
basis, kind, stage, direction,
centre { boundaries, members, entry, departure, retest },
entry, departure, rebound?, retest?, first_point_anchor?,
b2_chain?, candidate_stroke?, macd_comparison?
```

`b2_chain` is a finite `{first_point_anchor, rebound, first_retest}` relation. For forming B2 its `first_retest` is null and the candidate is separate. For `second_like`, the specific new canonical core starts with the concrete B2 retest and its entry is that chain's rebound. After the initial three core members it requires one rebound and the next actual retest, with strict new-core ZD/ZG and original first-point anchor guards. No direct parent mapping → no label. It never chains second-like labels or emits forming second-like results. This narrow canonical mapping is an implementation subset, not a general theory claim.

Natural `sdt/edt`, direction, high/low, actual endpoint prices and complete core member/boundary references provide operand identity. Role `position` values are one-based in the current analyzer list; legacy `overlap_bis` positions remain relative to the signal window. Positions can move with retention and are not persistent IDs. For confirmed stage observation, use the actual core and participating BI identities; for forming use its fixed rebound/departure anchor and core, not its changing endpoint/cutoff. The application records the first time that identity/stage is observed during its existing per-bar replay. No native timestamp here claims first-known time, and no native event database is introduced.

`di` is a slice of the current BI list, not a historical analyzer snapshot. Forming output is disabled for `di != 1` (`candidate_stroke.reason="di_is_not_history"`). Historical qualification and first-known age require replaying input prefixes. Same-dt corrections replace the current snapshot and can revise/revoke geometry and values. They do not create a new market observation time.

### Candidate stroke

`candidate_stroke` has `state=available|none|unavailable`, optional reason, `kind=reverse|extension`, direction, `anchor_at`, `anchor_price`, `anchor_price_at`, `end_price`, `end_price_at`, `end_newbar_at`, `as_of`, and `role=first_retest_candidate|other`.

The native analyzer supplies inclusion-processed NewBars and the last BI/FX. A reverse candidate excludes the entire anchor NewBar (including its merged raw constituents); an extension includes that centre. The boundary is selected from eligible NewBars with stable first-extreme ties. Only then is its actual raw price owner located. Raw extrema never substitute for `check_bi`. Missing real price ownership is `unavailable`.

Forming B2 requires the specific first-point→native rebound chain; forming B3 requires a native departure of the current latest core. Both require a current reverse candidate, strict raw protection, and no new high/low beyond that fixed rebound/departure anchor. Pure breakouts, extensions, old BIs exposed by withdrawal, and raw breaches cannot revive a first retest. A later new actual BI is evaluated afresh. This is a snapshot relationship, not a permanent ban on a symbol.

### Native MACD and raw interval facts

`macd.params=[12,26,9]`, `hist_scale=1`, `hist=DIF-DEA`. The existing TA-Lib-style native SMA-seeded EMA algorithm is the one numerical source. Indices 0–32 are warmup; the first fully ready triplet is index 33 (34th input). No doubled plotting convention is mixed into signal values.

```text
macd = {
  params, hist_scale, warmup_bars: 33,
  bars: [{dt, dif?, dea?, hist?}],
  coverage: {sdt?, edt?, sample_count, ready_count, first_ready_at?,
             input: "current_retained_bars_raw"},
  truncated: false
}
bis[].power = {
  sdt, edt, interval: "(sdt,edt]", sample_count,
  volume?, amount?, mean_volume?, mean_amount?, duration_seconds,
  macd: {positive_area?, negative_area?,
         dif_at_start?, dea_at_start?, hist_at_start?,
         dif_at_end?, dea_at_end?, hist_at_end?,
         sample_count, ready_count, status, reason?}
}
```

Areas sum positive/negative histogram magnitudes over exactly `(sdt,edt]`. Missing endpoints/known nested-BI raw coverage, missing warmup, or non-finite inputs/outputs make both areas null. The reason is `missing_coverage`, `warmup`, or `non_finite`. Ready numerical zero is preserved. A zero **entering same-direction area** makes the comparison `unavailable/zero_entry_area` with null ratio. A ready departure area smaller than the positive entry area is necessary for the first auxiliary; price, volume and duration contractions are not extra vetoes.

`macd_comparison` on first/second results includes both per-BI MACD operands, same-direction areas and ratio; second-like leaves it null because its current-core roles are different from the parent first-point operands; the associated role references identify the exact intervals. Model projections can look up all original values via `bis`, not recompute EMA. Volume and amount use original source units. Duration is elapsed natural seconds, not number of trading days. This code does not infer an exchange calendar or synthesize missing market bars.

The old long-series cache shortcut reseeded only a trailing window and rewrote five values. It is removed: the same retained input now yields identical values regardless of cache call history, same-dt revision or cold read. `TaCache.updated_macd_keys` still shares one calculation among signals in a computation round. Retention changes the available numerical origin; no values before that origin or their warmup are fabricated. `truncated=false` means the returned retained sequence was not further truncated, not that unbounded market history was retained.

### Existing signal references and compatibility

Existing `kind, window, overlap_bis, last_bi, zd, zg` remain. An appended `structure` contains the same evaluator's role-bearing result and stage. First/second signals now capture references too. A miss removes the key; unchanged label values still refresh changed operands. Old JSON/positional MessagePack snapshots decode with `structure=null` and missing endpoint prices null. Rebuild old hot state from raw input when adopting the new contract; decoding compatibility is not evidence that old cached labels have new semantics.

The Cargo workspace version is `1.0.1+ashare.20261029`; Cargo.lock and generated stub are synchronized. `uv.lock` contains a dynamic editable workspace node and has no duplicated CZSC version to edit (`uv lock --offline` remained unchanged). Consumers should pin the delivered Git commit, not a local checkout/path dependency.

## Validation and assumptions

Baseline `cargo test -p czsc-signals --tests`: 55 passed. The six actual alternating legs needed for a canonical third-point example fail the old MA/history entry, then pass the new owner even with `timeperiod=100000`. An isolated `git archive e864cfb` baseline run proves both failures with canonical geometry passing first: `target/acceptance/expected-red-baseline.log`. The final long-history example independently isolates MA as the only blocker; the six-leg example isolates the history gate. Canonical geometry code (`get_zs_seq`, `check_bi`, `remove_include`) is byte-unchanged from the baseline; existing golden geometry and reference boundary regressions remain.

Endpoint-only first-point fixtures now correctly fail MACD warmup. Geometry fixtures that need positive divergence were upgraded to dense, continuous raw input plus actual native warmup; expectations were not made green by filling missing areas with zeros. Equal-volume/equal-duration examples now pass solely on same-operand MACD. A stronger outgoing MACD area rejects otherwise identical geometry. Buy/sell mirror, boundary equality, interior wicks, latest-core replacement, first vs second retest, formed vs forming, anchor exceedance, withdrawal and fresh later BI cases are covered.

Native prefix tests exercise actual analyzer withdrawal/recovery, same-dt replacement and real NewBar price ownership. Wheel tests use reproducible `czsc.mock`, compare public signals with structure results, check all raw interval values, stages, 1/2-member tails, current membership, copy/replay, same-dt correction, and detached snapshots. Existing natural tail-revision fixture tests are also run; this is frozen engineering regression evidence, not live-market acceptance.

Independent numerical probe: TA-Lib 0.8.3, 7,320 mock bars / 230 retained-input snapshots; ready masks match and max absolute error is about `8.53e-14` (tolerance `1e-11`). Python 3.14's compensated `sum` can differ from Rust sequential addition by one ULP; wheel area assertions use `1e-12` tolerance rather than claim bit equality across different accumulation algorithms.

See `target/acceptance/` for baseline, expected RED, native regression, wheel tests, numerical oracle, build and lint logs. Build/test environments are local `.venv` and `target/chan-native`; no shared venv is modified. Natural daily full-pool recall, application first-known observation/sorting, downstream projection and actual trading efficacy remain application/main-agent acceptance work. No market-data fetch, model-paid replay, QMT call, push, merge or release was performed.

## Reproduce

Run from this checkout. `-I` and pytest importlib mode deliberately test the installed wheel rather than the source tree's stub-only `czsc._native` directory.

```sh
UV_PROJECT_ENVIRONMENT="$PWD/.venv" uv sync --locked --no-install-project
PYO3_PYTHON=/opt/homebrew/opt/python@3.14/bin/python3.14 CARGO_TARGET_DIR="$PWD/target/chan-native" cargo test -p czsc -p czsc-core -p czsc-signals -p czsc-trader --lib --tests
PYO3_PYTHON=/opt/homebrew/opt/python@3.14/bin/python3.14 CARGO_TARGET_DIR="$PWD/target/chan-native" cargo run --bin stub_gen -p czsc-python --no-default-features --features stub-gen
PYO3_PYTHON=/opt/homebrew/opt/python@3.14/bin/python3.14 CARGO_TARGET_DIR="$PWD/target/chan-native" .venv/bin/maturin build --release --out target/wheels-release
uv pip install --python .venv/bin/python --reinstall --no-deps target/wheels-release/czsc-1.0.1+ashare.20261029-cp310-abi3-macosx_11_0_arm64.whl
.venv/bin/python -I -m pytest --import-mode=importlib tests/unit
uv pip install --python .venv/bin/python TA-Lib==0.8.3
.venv/bin/python -I scripts/verify_native_structure.py --talib
.venv/bin/python -I scripts/benchmark_structure_state.py --rounds 2 --warmup 200 --example-out target/acceptance/light-daily-example.json
```

`TA-Lib` is an isolated test oracle only, not a runtime dependency. The wheel is macOS arm64; the package still requires Python >=3.14 even though PyO3's extension ABI floor is cp310. Linux consumers must build the same Git commit on their platform.

## Daily prefix measurement (light supplement)

Measured through the installed optimized wheel on macOS arm64 / Python 3.14.7 / wbt 0.2.1. The stable `czsc.mock.generate_symbol_kines("000001", "日线", "20100101", "20250101", seed=7)` sample has **3,914 bars**, frame SHA-256 `1356a23dec5f817459c1a4fbb23902246946c517d8b4621a3d3319326c92d2be`. Each run creates its own analyzer (`max_bi_num=50, min_bi_len=6`), warms up with 200 bars and then makes **3,714 sequential update calls and 3,714 read calls**, without sampling/skipping prefixes. Order is full→light, then light→full. Four runs total 14,856 measured update/read pairs, plus four final full-package reads outside the timer. Final retained material is 782 raw bars / 50 BIs.

| Round / view | Total update + read | P50 / P95 update + read | P50 / P95 read alone |
| --- | ---: | ---: | ---: |
| 1 full | 7,106.64 ms | 1.968 / 2.357 ms | 1.963 / 2.351 ms |
| 1 light | 3,725.18 ms | 1.038 / 1.277 ms | 1.036 / 1.274 ms |
| 2 light | 3,672.81 ms | 1.027 / 1.257 ms | 1.024 / 1.252 ms |
| 2 full | 6,799.60 ms | 1.911 / 2.210 ms | 1.907 / 2.208 ms |

| View (identical bytes both rounds) | Sum across 3,714 returns | P50 / P95 bytes | Maximum / last bytes |
| --- | ---: | ---: | ---: |
| full | 456,415,920 | 125,566.5 / 145,018.2 | 151,526 / 140,684 |
| light | 26,220,945 | 6,503 / 11,946 | 18,454 / 16,031 |

The timed calls include native numerical/structure computation, Rust serde and PyO3 JSON decoding. Initialization, Python byte-count serialization, equality hashing, counters, deletion of returned dicts and the extra final full package are excluded. The JSON report separately records complete loop wall time including bookkeeping. Bytes are UTF-8 JSON with compact separators and `ensure_ascii=False`, measured for every return; they are not a peak-RSS measurement and use a different whitespace format from the original 147,137-byte single-snapshot probe.

All four runs have the same ordered prefix-state digest `b6a302bda4eed1dff0b46f6ce6a640ae9e6a7beb5a6495e74246610602490af6`, also identical to the pre-supplement `a60fc7d` wheel on these prefixes. This covers all nine light fields and their entire nested failed/eligible references, not labels alone. Confirmed first/second/third and forming second/third in both directions are present. The prior complete `seed=7` 30-minute prefix-982 second-like snapshot also remains exactly equal, including all BI and per-bar numerical output. Rust and wheel regression separately cover same-dt replacement/restoration, cold prefix replay, retained-input pruning, missing coverage, warmup/non-finite/zero-area material, di slices and forming-vs-confirmed qualification.

Use `target/acceptance/light-daily-example.json` for a final daily light view and `light-mock-seed7-prefix982.json` for the finite second-like reference example. The benchmark script contains no EMA, signal or geometry implementation.

Limit: this is a single local serial mock stream, not natural quotes, a 4,000-symbol pool, concurrent workers, the application's observer/storage, or a premarket wall-clock acceptance test. Light still spends about 1 ms per read here computing qualification MACD and canonical relations/references. That remaining cost has not been declared irreducible or extrapolated into a full-pool pass; application budget validation must measure its actual integration. No signal-policy shortcut or approximate long-lived cache was introduced to improve the numbers.

## Code organization and remaining boundaries

The reviewed CXT first/second/third functions were extracted into `cxt_points.rs`; numerical MACD into `utils/macd.rs`; interval values into `utils/bi_power.rs`; snapshot projection into `structure.rs`; core tail geometry into `analyze/candidate.rs`. The pre-existing `cxt.rs` and `utils/ta.rs` still exceed 1,200 lines because they contain unrelated registered signal/TA families. This batch removes the owned responsibilities rather than renaming unrelated code into `part1`/wrapper files. The new owner files stay below 800 lines.

The supplement keeps both views in the same 238-line `structure.rs` owner. The touched `czsc/_native/__init__.pyi` exceeds 1,200 lines because it is the single generated native-module stub; it is regenerated by `stub_gen`, not manually split or independently maintained.

Full retained MACD plus all BI/core references intentionally cost more bytes than a compact model prompt. Use `get_structure_state` for each observed prefix and `get_structure` for the final needed values; project only relevant roles/ranges with explicit coverage, without silently recomputing or filling missing history. The standalone probes record actual time and output bytes; they are not full-market throughput or production memory claims.

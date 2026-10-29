# czsc-signals

> 💡 **大多数用户应优先使用 facade crate [`czsc`](https://crates.io/crates/czsc)**：
> `cargo add czsc` 一行拿到全部公共 API，本 crate 已在 `czsc::signals`
> 命名空间中 re-export。仅当你**只想接入 czsc 的信号函数库**到自己的
> 框架（不用 czsc-trader 状态机）时才单独依赖本 crate。

缠论信号函数库 —— 30+ 量化信号函数，按类别分组：

- `bar`：基础 K 线形态信号
- `cxt`：缠论上下文信号（笔、中枢相关）
- `tas`：技术分析叠加信号（MACD、RSI 等）
- `vol`：成交量信号
- `pressure` / `obv` / `cvolp`：资金、压力、量价信号

每个信号通过 [`#[signal(...)]`](https://crates.io/crates/czsc-signal-macros)
属性宏在编译期自动注册到 inventory，运行期可按字符串配置（如
`"日线_D1#10_看多V240520"`）解析、调用。

## 用法

```toml
[dependencies]
czsc-signals = "1.0"
```

```rust
// 通过 inventory 拿到注册的信号函数
let funcs = czsc_signals::all_signals();
println!("registered: {}", funcs.len());
```

## BS1 / BS2 / BS3 的结构与升级合同

`1.0.1+ashare.20261029` 保留已审五个 CXT 函数及参数、Signal key、四段 value 形状，更新辅助命中语义。`cxt_points.rs` 的公开值与 `signal_references`、`get_structure` 共用 `utils/bs.rs` 求值；Python 不设过滤层。

- 一买/一卖：同 canonical 核的真实进入/离开、同向、新极值和端点守界保持；用同操作数原生 MACD(12,26,9) 同向柱面积减弱替换价幅 AND 量/时长收缩。`hist_scale=1`，每笔 `(sdt,edt]`；缺预热/覆盖、非有限、零进入面积明确不可判定。量、幅度、时长仅作原值。`basis=bi_proxy`，面积减弱不独自证明严格缠论一买。
- 二买/二卖：具体一类锚→紧邻反弹→首次回试，维持本项目严格守锚子集，逐 raw 守界。移除 MA 否决，MA 参数只保留原键兼容。
- 三买/三卖：只用 `finished_bis` 最新有效 canonical 核；一两笔尾组不遮住它，新合法核产生后不能选择旧核。从离开高/低点之后逐已收 raw 严格守 ZG/ZD，突破笔内部不套此条件。移除 MA 及经验总笔数门，材料由核、离开、首次回试关系决定。
- `get_structure` 单独给出 `confirmed_valid` / `forming_retest`。后者仅允许二/三类原生反弹/离开锚下的当前首次 reverse 候选，不能冒充完整 BI 或公开已确认信号；新高/低、撤锚及 raw 破界都会重新求值。一类无 forming；类二仅在直接可定位的有限 B2 链及其具体新核内给 confirmed 结果。

几何仍由 `czsc-core::get_zs_seq` 独占管理，见 [核心分组合同](../czsc-core/README.md)。本次不改 canonical 分组。信号中的进入/核心/离开/回试由该分组定位，不累计成员数量推算，也不实现跨周期递归。

`CzscSignals.s` 不变。原 `signal_references` 六字段保留，新增 `structure`，一/二类也导出引用。`overlap_bis` 是同核初始三笔；位置相对各自窗口，一/二类窗口从真实进入笔开始，三类从核心开始。引用同值换操作数时刷新、失去命中即清除。旧 JSON/MessagePack 缺新字段时读为 null，不补零。

Python 新增 `czsc.get_structure(c, di=1)`；Rust 为 `czsc::get_structure(&c, 1)`。它给末两有效核、1/2 笔尾组、NewBar 候选、角色引用、所有当前 BI 和原生逐 bar/逐笔 MACD。`finished_bis` 仅是当前成员口径，不能叫不可重画确认。

`di>1` 是当前列表切片，不是历史快照；forming 禁用。首次可知由应用逐 K 观察记录，不能拿端点时间倒填。升级时从 raw 重建热状态；旧快照可解码不等于旧标签已具有新语义。其它历史版本策略不属于本次已审五入口。

字段、有限类二子集、数值缺失、缓存精度、构建和资源限制见 [完整合同及验收](../../docs/chan-native-structure-20261029.md)。

## 项目主页

- 仓库：<https://github.com/waditu/czsc>
- Python 入口：`czsc._native.signals`

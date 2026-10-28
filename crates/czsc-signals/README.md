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

`1.0.1+ashare.20261028` 保留原五个 CXT 函数及参数、Signal key、四段 value 形状；命中集合与窗口笔数改变。Rust 和 Python 共用同一 Rust 判定，Python 不设过滤层。

- 一买/一卖：至少一个有效 canonical 笔中枢；可观察进入笔与离开笔同向，离开创新低/新高，价差严格衰减且成交量或笔长严格衰减。单中枢盘整背驰可命中，不强制双中枢。按方向选择进入笔：中枢首成员同向时用该成员，否则用中枢之前一笔；不依赖该中枢是否位于保留历史的开头。首成员作为进入笔时，须从区间外进入，之后另有三笔共同重叠。这是笔级背驰代理，不是递归走势级别背驰证明，也不要求单独的 MACD 信号同拍命中。
- 二买/二卖：在同一当前结构上重新验证前一买/一卖，只允许其后反向一笔加首次回抽，严格不破锚点，期间原始价格也不能破界。原 SMA/EMA 参数及均线过滤保留；普通中枢突破后的回抽不单独冒充二买。
- 三买/三卖：对 `finished_bis` 使用核心 `get_zs_seq`，反向选取最新 `is_valid() && len(bis)>=3` 中枢；尾部一两笔不遮住它。中枢形成后紧邻离开及首次回抽，回抽与其后原始 K 线严格处于 ZG 上方或 ZD 下方。离开笔可包含于中枢，因此不以包含自身的 GG/DD 作突破门槛。`third_bs` 继续比较中枢第1、3笔及回抽笔终点均线。

`CzscSignals.s` 的 API 不变。`signal_references` 保持 `kind/window/overlap_bis/last_bi/zd/zg` 六个字段；新 kind 为 `canonical_zs`，初始三笔是 `overlap_bis`，window 覆盖中枢起点到当前回抽。旧 kind 可解码但不再产生；消费者若穷举 kind 需增加该值。引用在同值换操作数时刷新，失去命中即清除。

`di>1` 只选择当前笔序列中的历史位置，不重建历史当时状态。第二/三类只检查目标笔之前的结构和截至目标端点的原始路径；只有 `di=1` 检查到当前输入。需要历史首次可知时间时必须逐 K 回放，不能用最终结构倒填。

升级时由原始 K 线重建状态，并重算旧回测；不复用跨版本 Rust 热快照内的旧 `s` 与频率缓存。现有同尾 K 撤销、TA 缓存、Clone/Serde 与 Python copy/pickle 重建边界保持。`min_bis` 仍只是最少启动评估笔数，不保证足以形成命中结构。

## 项目主页

- 仓库：<https://github.com/waditu/czsc>
- Python 入口：`czsc._native.signals`

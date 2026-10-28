# czsc-core

> 💡 **大多数用户应优先使用 facade crate [`czsc`](https://crates.io/crates/czsc)**：
> `cargo add czsc` 一行拿到全部公共 API，本 crate 中的核心类型已在 facade
> 顶层 re-export。仅当你**只想要缠论核心算法**、不需要工具/信号/trader 时
> 才单独依赖本 crate。

缠论（CZSC，缠中说禅技术分析理论）核心数据结构与算法的 Rust 实现。

提供分型（`FX`）、笔（`BI`）、中枢（`ZS`）、`CZSC` 分析器等核心类型，以及 K 线
包含关系处理（`remove_include`）、笔识别（`check_bi`）等基础算法。

## 用法

```toml
[dependencies]
czsc-core = "1.0"
```

```rust
use czsc_core::objects::{RawBar, Freq};
use czsc_core::analyze::CZSC;

let bars: Vec<RawBar> = /* ... */;
let czsc = CZSC::new(bars, 50, 6);
println!("最新一笔方向: {:?}", czsc.bi_list.last().map(|b| b.direction));
```

`update_bar` 接收与末根 K 线相同的 `dt` 时，完整替换该根输入；包含关系、分型和笔
与该版本只输入一次的结果一致。分析器只保存最近一次输入前的未完成笔状态，以及该次
更新删除或裁剪的数据；下一根输入会替换这份状态，不积累历史快照。

`Clone`、Serde 和 Python pickle 均保留这份修订状态。Serde 快照必须包含
`tail_update` 字段；旧快照缺失该字段时会在反序列化时返回错误，不能从已裁剪的
`bars_raw` 恢复。旧 pickle 仅包含构造参数，仍按旧参数重建，无法补回此前丢失的状态。
`CZSCBuilder` 仅支持空分析状态；分析现有 K 线请调用 `CZSC::new`。

## 特性

- `python`：启用 PyO3 binding，供 `czsc-python` crate 聚合到 `czsc._native` 扩展。

## 项目主页

- 仓库：<https://github.com/waditu/czsc>
- Python 包：[`czsc` on PyPI](https://pypi.org/project/czsc/)

## Canonical 中枢分组

`analyze::utils::get_zs_seq` 是 `CZSC.zs_list`、Python同名函数和信号的唯一分组入口；`ZS::new` 仍对调用方给定的核心成员计算边界，不自行猜测哪笔是进入段。

- 保留历史的首笔作进入上下文，首核最早从下一笔开始。连续三笔无重叠时，按两笔推进上下文寻找下一核心；未成三笔的尾组不作有效中枢。
- 成立后前三核心笔固定 ZD/ZG。返回核心的笔计入延伸；首次严格不返回的回试确认离开。
- 仅在新回试与前两笔重叠、整个新重叠严格位于旧核之外、且旧组仍保留最初三笔时，将旧尾两笔移交新核。成员不复用，旧已成立核保持有效。
- 无法回退时，外侧回试为新核首笔，前一离开笔为其进入上下文。进入笔不属于它进入的核心，但可同时是前一核心的离开成员。首部或寻核期间跳过的笔不计入 ZS，不能用分组长度累计推断原笔索引。

这是保留 BI 上下文中的确定性分解，未实现递归走势级别或高级别中枢扩张。历史裁剪会改变初始化上下文，不承诺跨 `max_bi_num` 裁剪保持最早核心不变；历史判定须逐K重放。`1.0.1+ashare.20261028` 升级改变公开中枢边界及成员，依赖结构止损的消费者也必须从 K 线重建并重新验证。

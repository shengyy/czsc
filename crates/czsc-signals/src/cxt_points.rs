use crate::{
    params::ParamView,
    reference::SignalReference,
    types::TaCache,
    utils::{
        bs::{self, Evaluation},
        macd::{STRUCTURE_MACD_KEY, update_macd_cache},
        sig::{get_usize_param, make_kline_signal_v1, make_kline_signal_v2},
    },
};
use czsc_core::{
    analyze::CZSC,
    objects::{direction::Direction, signal::Signal},
};
use czsc_signal_macros::signal;

fn publish(c: &CZSC, di: usize, key: &str, e: Option<&Evaluation>, cache: &mut TaCache) {
    cache.signal_references.remove(key);
    if let Some(reference) = e.and_then(|e| SignalReference::from_evaluation(c, di, e)) {
        cache.signal_references.insert(key.into(), reference);
    }
}
fn first(c: &CZSC, p: &ParamView, cache: &mut TaCache, direction: Direction) -> Vec<Signal> {
    let di = get_usize_param(p, "di", 1);
    let k1 = c.freq.to_string();
    let k2 = format!("D{di}B");
    let (k3, label) = if direction == Direction::Down {
        ("BUY1", "一买")
    } else {
        ("SELL1", "一卖")
    };
    update_macd_cache(c, STRUCTURE_MACD_KEY, 12, 26, 9, cache);
    let e = bs::prefix_end(c, di)
        .map(|end| bs::evaluate_first(c, end, direction, &cache.macd[STRUCTURE_MACD_KEY]));
    publish(c, di, &format!("{k1}_{k2}_{k3}"), e.as_ref(), cache);
    if let Some(e) = e
        && e.confirmed()
    {
        let p = e.point.unwrap();
        return make_kline_signal_v2(&k1, &k2, k3, label, &format!("{}笔", p.end - p.start));
    }
    make_kline_signal_v1(&k1, &k2, k3, "其他")
}

/// cxt_first_buy_V221126：同 canonical 核进入/离开笔 MACD 同向面积减弱辅助。
/// 保留真实进入、离开、新低和端点守界。hist = DIF - DEA；缺预热/覆盖
/// 或零进入面积不可判定。basis=bi_proxy，不等于严格递归缠论一买。
/// Signal('60分钟_D1B_BUY1_一买_5笔_任意_0')
#[signal(
    category = "kline",
    name = "cxt_first_buy_V221126",
    template = "{freq}_D{di}B_BUY1V221126",
    opcode = "CxtFirstBuyV221126",
    param_kind = "CxtFirstBuyV221126"
)]
pub fn cxt_first_buy_v221126(c: &CZSC, params: &ParamView, cache: &mut TaCache) -> Vec<Signal> {
    first(c, params, cache, Direction::Down)
}

/// cxt_first_sell_V221126：一买辅助的镜像；仅输出当前完整 BI 关系。
/// Signal('60分钟_D1B_SELL1_一卖_5笔_任意_0')
#[signal(
    category = "kline",
    name = "cxt_first_sell_V221126",
    template = "{freq}_D{di}B_SELL1V221126",
    opcode = "CxtFirstSellV221126",
    param_kind = "CxtFirstSellV221126"
)]
pub fn cxt_first_sell_v221126(c: &CZSC, params: &ParamView, cache: &mut TaCache) -> Vec<Signal> {
    first(c, params, cache, Direction::Up)
}

/// cxt_second_bs_V230320：具体一买/卖锚、紧邻反弹、首次回试及严格守锚子集。
/// ma_type/timeperiod 仅保留原信号键兼容；不再计算或施加 MA 条件。
/// 未成 BI 的研究候选仅由 get_structure 的 forming_retest 输出。
/// Signal('60分钟_D1#SMA#21_BS2辅助V230320_二买_任意_任意_0')
#[signal(
    category = "kline",
    name = "cxt_second_bs_V230320",
    template = "{freq}_D{di}#{ma_type}#{timeperiod}_BS2辅助V230320",
    opcode = "CxtSecondBsV230320",
    param_kind = "CxtSecondBsV230320"
)]
pub fn cxt_second_bs_v230320(c: &CZSC, params: &ParamView, cache: &mut TaCache) -> Vec<Signal> {
    let di = get_usize_param(params, "di", 1);
    let period = get_usize_param(params, "timeperiod", 21);
    let ma = params.str("ma_type", "SMA").to_uppercase();
    let k1 = c.freq.to_string();
    let k2 = format!("D{di}#{ma}#{period}");
    let k3 = "BS2辅助V230320";
    update_macd_cache(c, STRUCTURE_MACD_KEY, 12, 26, 9, cache);
    let e = bs::prefix_end(c, di).map(|end| {
        bs::evaluate_second(
            c,
            end,
            c.bi_list[end - 1].direction,
            &cache.macd[STRUCTURE_MACD_KEY],
            false,
        )
    });
    publish(c, di, &format!("{k1}_{k2}_{k3}"), e.as_ref(), cache);
    let label = match e.as_ref().filter(|e| e.confirmed()).map(|e| e.direction) {
        Some(Direction::Down) => "二买",
        Some(Direction::Up) => "二卖",
        None => "其他",
    };
    make_kline_signal_v1(&k1, &k2, k3, label)
}

/// cxt_third_bs_V230318：最新合法 canonical 核离开后的首次独立回试。
/// 从离开端点之后逐 raw 严格守 ZG/ZD，并拒绝锚后再创新高/低。
/// ma_type/timeperiod 仅保留信号键兼容，没有 MA 或经验笔数门。
/// Signal('60分钟_D1#SMA#34_BS3辅助V230318_三买_任意_任意_0')
#[signal(
    category = "kline",
    name = "cxt_third_bs_V230318",
    template = "{freq}_D{di}#{ma_type}#{timeperiod}_BS3辅助V230318",
    opcode = "CxtThirdBsV230318",
    param_kind = "CxtThirdBsV230318"
)]
pub fn cxt_third_bs_v230318(c: &CZSC, params: &ParamView, cache: &mut TaCache) -> Vec<Signal> {
    let di = get_usize_param(params, "di", 1);
    let period = get_usize_param(params, "timeperiod", 34);
    let ma = params.str("ma_type", "SMA").to_uppercase();
    let k1 = c.freq.to_string();
    let k2 = format!("D{di}#{ma}#{period}");
    let k3 = "BS3辅助V230318";
    let e = bs::prefix_end(c, di)
        .map(|end| bs::evaluate_third(c, end, c.bi_list[end - 1].direction, false));
    publish(c, di, &format!("{k1}_{k2}_{k3}"), e.as_ref(), cache);
    let label = match e.as_ref().filter(|e| e.confirmed()).map(|e| e.direction) {
        Some(Direction::Down) => "三买",
        Some(Direction::Up) => "三卖",
        None => "其他",
    };
    make_kline_signal_v1(&k1, &k2, k3, label)
}

/// cxt_third_buy_V230228：同一三类求值的买侧；材料由有效核、离开和回试决定。
/// di 是当前列表切片，不是历史快照；历史可知时点须逐输入前缀重放。
/// Signal('60分钟_D1_三买辅助V230228_三买_5笔_任意_0')
#[signal(
    category = "kline",
    name = "cxt_third_buy_V230228",
    template = "{freq}_D{di}_三买辅助V230228",
    opcode = "CxtThirdBuyV230228",
    param_kind = "CxtThirdBuyV230228"
)]
pub fn cxt_third_buy_v230228(c: &CZSC, params: &ParamView, cache: &mut TaCache) -> Vec<Signal> {
    let di = get_usize_param(params, "di", 1);
    let k1 = c.freq.to_string();
    let k2 = format!("D{di}");
    let k3 = "三买辅助V230228";
    let e = bs::prefix_end(c, di).map(|end| bs::evaluate_third(c, end, Direction::Down, false));
    publish(c, di, &format!("{k1}_{k2}_{k3}"), e.as_ref(), cache);
    if let Some(e) = e
        && e.confirmed()
    {
        let p = e.point.unwrap();
        return make_kline_signal_v2(&k1, &k2, k3, "三买", &format!("{}笔", p.end - p.start));
    }
    make_kline_signal_v2(&k1, &k2, k3, "其他", "其他")
}

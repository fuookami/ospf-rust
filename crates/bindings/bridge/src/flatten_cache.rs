//! 续挖第 8 单：平展缓存的三条路径（`model/flatten/flatten_context` + `flattenable`）
//! Continuation, delivery 8: the three paths through the flatten cache.
//!
//! `flatten_context.rs` / `flattenable.rs` 一共 11 个未引用类型，但它们不是 11 个独立的东西：`FlattenContextTrait`
//! 的**默认方法**就是全部语义。这一单把它们逐一变成可断言的事实：
//!
//! | 路径 | 实测行为 |
//! | --- | --- |
//! | `flatten_cached` | 命中就直接返回缓存，**不调用 `flatten`**；未命中则平展并写入 |
//! | `flatten_force` | **总是**重新平展并**更新**缓存（后续 `flatten_cached` 看到的是新值） |
//! | `clear_polynomial` / `clear_symbol` / `clear_monomial` | 返回"是否清到了"的布尔，三张缓存**各管各的** |
//! | `are_dependencies_cached` | 只有**全部**依赖在多项式缓存里才为真 |
//! | `clear_dependency_caches` | 逐个 `clear_polynomial`，其他缓存不动 |
//! | `cache_size` / `is_cache_empty` | 三张缓存之和 / 三者皆空 |
//!
//! 三条最容易踩的坑，这一单都钉住：
//!
//! 1. **`flatten_cached` 用 `identifier()` 当键**。两个对象只要 `identifier()` 相同就**共用同一个槽**——第二个对象
//!    会拿到第一个的缓存值，而 `flatten` 根本不会被调用。这是"标识符必须唯一"的真实后果，不是文档修辞。
//! 2. **`flatten_force` 之后缓存是新值**。它先算再写，所以不是"跳过缓存"，而是"覆盖缓存"。
//! 3. **`clear_polynomial` 不影响 symbol / monomial 缓存**。三张缓存互不代管，只清一张不会让另两张变空。
//!
//! 桥接不重写任何缓存逻辑：全部走 `FlattenContextTrait` / `Flattenable` / `FlattenableWithDependencies` 的默认方法，
//! 这一层只实现一个 `VecTokenList` 支撑的上下文与一个可数的测试符号。
//!
//! The eleven unreferenced types there are not eleven independent things: the **default methods** of `FlattenContextTrait`
//! are the whole semantics, and this delivery turns each into an assertable fact — `flatten_cached` returns the cached value
//! **without calling `flatten`** on a hit and flattens plus stores on a miss; `flatten_force` **always** re-flattens and
//! **overwrites** the cache; the three `clear_*` methods return whether they hit, and the three caches are **independent**;
//! `are_dependencies_cached` is true only when **every** dependency sits in the polynomial cache; `clear_dependency_caches`
//! clears those and nothing else. Three traps are pinned down: **`flatten_cached` keys on `identifier()`**, so two objects
//! sharing an identifier **share one slot** and the second gets the first's cached value with `flatten` never running — the
//! real consequence of "identifiers must be unique", not documentation rhetoric; **after `flatten_force` the cache holds the
//! new value**, since it computes then writes rather than skipping the cache; and **`clear_polynomial` leaves the symbol and
//! monomial caches untouched**. The bridge rewrites no cache logic: everything goes through the trait default methods, and this
//! layer only supplies a `VecTokenList`-backed context and a countable test symbol.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use ospf_rust_core::model::flatten::{
    FlattenContextTrait, Flattenable, FlattenableWithDependencies, FlattenedMonomial,
    FlattenedPolynomial, FlattenedSymbol, Identified,
};
use ospf_rust_core::token::VecTokenList;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个测试符号：平展结果就是它携带的值，`flatten` 被调用一次计数就加一。
/// A test symbol whose flatten result is the value it carries; each `flatten` call bumps the counter.
struct Counted {
    identifier: u64,
    value: f64,
    dependencies: Vec<u64>,
    calls: Arc<AtomicUsize>,
}

impl Identified for Counted {
    fn identifier(&self) -> u64 { self.identifier }
}

impl Flattenable<TestContext, f64> for Counted {
    fn flatten(&self, _ctx: &mut TestContext) -> f64 {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.value
    }
}

impl FlattenableWithDependencies<TestContext, f64> for Counted {
    fn dependencies(&self) -> Vec<u64> { self.dependencies.clone() }
}

/// 平展上下文：三张缓存各自独立，行为全走 trait 默认方法。
/// The flatten context: three independent caches, all behaviour from the trait's default methods.
struct TestContext {
    tokens: VecTokenList<f64>,
    monomials: HashMap<u64, FlattenedMonomial<f64>>,
    polynomials: HashMap<u64, FlattenedPolynomial<f64>>,
    symbols: HashMap<u64, FlattenedSymbol<f64>>,
}

impl TestContext {
    fn new() -> Self {
        Self { tokens: VecTokenList::new(), monomials: HashMap::new(),
            polynomials: HashMap::new(), symbols: HashMap::new() }
    }
}

impl FlattenContextTrait<f64> for TestContext {
    type Polynomial = f64;
    type Monomial = f64;
    fn token_list(&self) -> &dyn ospf_rust_core::token::TokenList<f64> { &self.tokens }
    fn monomial_cache(&self) -> &HashMap<u64, FlattenedMonomial<f64>> { &self.monomials }
    fn monomial_cache_mut(&mut self) -> &mut HashMap<u64, FlattenedMonomial<f64>> { &mut self.monomials }
    fn polynomial_cache(&self) -> &HashMap<u64, FlattenedPolynomial<f64>> { &self.polynomials }
    fn polynomial_cache_mut(&mut self) -> &mut HashMap<u64, FlattenedPolynomial<f64>> {
        &mut self.polynomials
    }
    fn symbol_cache(&self) -> &HashMap<u64, FlattenedSymbol<f64>> { &self.symbols }
    fn symbol_cache_mut(&mut self) -> &mut HashMap<u64, FlattenedSymbol<f64>> { &mut self.symbols }
}

/// 一个符号的定义。 / A symbol definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlattenSymbolSpec {
    /// 唯一标识符（`flatten_cached` 的缓存键）。 / The unique identifier (the `flatten_cached` key).
    pub identifier: u64,
    /// 平展结果。 / The flatten result.
    pub value: f64,
    /// 依赖的其他符号标识符。 / The dependent symbol identifiers.
    #[serde(default)]
    pub dependencies: Vec<u64>,
}

/// 一条缓存操作。 / One cache operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlattenCacheOperation {
    /// `flattenCached` / `flattenForce` / `areDependenciesCached` / `clearDependencyCaches` /
    /// `clearPolynomial` / `clearSymbol` / `clearMonomial` / `clearItems` / `clear` / `cacheSize` /
    /// `isCacheEmpty`。 / The operation.
    pub operation: String,
    /// 参与该操作的符号（`flatten*` / `areDependenciesCached` / `clearDependencyCaches` 用）。
    /// The symbol the operation works on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub symbol: Option<FlattenSymbolSpec>,
    /// 参与该操作的标识符（`clear*` / `clearItems` 用）。 / The identifiers the operation works on.
    #[serde(default)]
    pub identifiers: Vec<u64>,
}

/// 平展缓存请求。 / The flatten-cache request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlattenCacheRequest {
    /// 操作序列。 / The operation log.
    #[serde(default)]
    pub operations: Vec<FlattenCacheOperation>,
}

/// 一条操作的结论。 / The verdict for one operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlattenCacheRow {
    /// 操作名。 / The operation name.
    pub operation: String,
    /// 返回的多项式值（`flatten*` 有）。 / The returned polynomial (`flatten*` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<f64>,
    /// 真值结论（`areDependenciesCached` / `isCacheEmpty` 有）。 / The boolean verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub flag: Option<bool>,
    /// 清除是否命中（`clearPolynomial` / `clearSymbol` / `clearMonomial` 有）。 / Whether a clear hit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub removed: Option<bool>,
    /// 缓存总数（`cacheSize` 有）。 / The total cache size (`cacheSize` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cache_size: Option<u32>,
    /// 这一条操作里 `flatten` 是否真的被调用过（计数增减）。 / Whether `flatten` actually ran here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub flattened: Option<bool>,
    /// 操作后的缓存总数。 / The total cache size after the operation.
    pub length_after: u32,
    /// 操作后的多项式缓存大小。 / The polynomial cache size after the operation.
    pub polynomials_after: u32,
}

/// 平展缓存结论。 / The flatten-cache verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlattenCacheReport {
    /// 操作条数。 / The number of operations.
    pub operation_count: u32,
    /// 逐条结论。 / Each operation's verdict.
    pub operations: Vec<FlattenCacheRow>,
    /// 最终缓存总数。 / The final total cache size.
    pub final_cache_size: u32,
    /// 最终是否为空。 / Whether the cache ends empty.
    pub final_is_empty: bool,
    /// `flatten` 的总调用次数（`flatten_cached` 命中时不该增加）。 / The total `flatten` calls.
    pub flatten_calls: u32,
    /// 操作词表（桥接定义）。 / The operation vocabulary (bridge-defined).
    pub operation_vocabulary: Vec<String>,
}

/// 操作上限（信任边界）。 / The operation cap (a trust boundary).
const MAX_OPERATIONS: usize = 4_096;

/// 回放平展缓存操作并给出可断言的结论。非法输入是**错误**。
/// Replay the flatten-cache operations and return assertable verdicts. Illegal input is an **error**.
pub fn verify_flatten_cache(request: &FlattenCacheRequest) -> Result<FlattenCacheReport> {
    if request.operations.len() > MAX_OPERATIONS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "flatten cache failed validation: at most {MAX_OPERATIONS} operations are accepted")));
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let mut context = TestContext::new();
    let mut rows = Vec::with_capacity(request.operations.len());
    for operation in &request.operations {
        rows.push(replay(operation, &mut context, &calls)?);
    }
    Ok(FlattenCacheReport {
        operation_count: u32::try_from(rows.len()).unwrap_or(u32::MAX),
        operations: rows,
        final_cache_size: size(&context),
        final_is_empty: context.is_cache_empty(),
        flatten_calls: u32::try_from(calls.load(Ordering::Relaxed)).unwrap_or(u32::MAX),
        operation_vocabulary: vec!["flattenCached".to_owned(), "flattenForce".to_owned(),
            "areDependenciesCached".to_owned(), "clearDependencyCaches".to_owned(),
            "clearPolynomial".to_owned(), "clearSymbol".to_owned(), "clearMonomial".to_owned(),
            "clearItems".to_owned(), "clear".to_owned(), "cacheSize".to_owned(),
            "isCacheEmpty".to_owned()],
    })
}

/// 当前缓存总数。 / The current total cache size.
fn size(context: &TestContext) -> u32 {
    u32::try_from(context.cache_size()).unwrap_or(u32::MAX)
}

/// 当前多项式缓存大小。 / The current polynomial cache size.
fn polynomials(context: &TestContext) -> u32 {
    u32::try_from(context.polynomial_cache().len()).unwrap_or(u32::MAX)
}

/// 回放一条操作。 / Replay one operation.
fn replay(operation: &FlattenCacheOperation, context: &mut TestContext,
    calls: &Arc<AtomicUsize>) -> Result<FlattenCacheRow>
{
    let name = operation.operation.as_str();
    let symbol = || -> Result<Counted> {
        let spec = operation.symbol.as_ref().ok_or_else(|| BridgeError::new("InvalidInput",
            &format!("flatten cache failed validation: operation `{name}` needs a symbol")))?;
        Ok(Counted { identifier: spec.identifier, value: spec.value,
            dependencies: spec.dependencies.clone(), calls: Arc::clone(calls) })
    };
    let mut value = None;
    let mut flag = None;
    let mut removed = None;
    let mut cache_size = None;
    let mut flattened = None;
    match name {
        "flattenCached" => {
            let before = calls.load(Ordering::Relaxed);
            value = Some(symbol()?.flatten_cached(context));
            flattened = Some(calls.load(Ordering::Relaxed) != before);
        }
        "flattenForce" => {
            let before = calls.load(Ordering::Relaxed);
            value = Some(symbol()?.flatten_force(context));
            flattened = Some(calls.load(Ordering::Relaxed) != before);
        }
        "areDependenciesCached" => flag = Some(symbol()?.are_dependencies_cached(context)),
        "clearDependencyCaches" => {
            symbol()?.clear_dependency_caches(context);
        }
        "clearPolynomial" => removed = Some(one(context.clear_polynomial(identifier(operation, name)?))),
        "clearSymbol" => removed = Some(one(context.clear_symbol(identifier(operation, name)?))),
        "clearMonomial" => removed = Some(one(context.clear_monomial(identifier(operation, name)?))),
        "clearItems" => context.clear_items(&operation.identifiers),
        "clear" => context.clear(),
        "cacheSize" => cache_size = Some(size(context)),
        "isCacheEmpty" => flag = Some(context.is_cache_empty()),
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "flatten cache failed validation: unknown operation `{other}`"))),
    }
    Ok(FlattenCacheRow { operation: name.to_owned(), value, flag, removed, cache_size, flattened,
        length_after: size(context), polynomials_after: polynomials(context) })
}

/// 取操作里唯一需要的标识符。 / Take the single identifier the operation needs.
fn identifier(operation: &FlattenCacheOperation, name: &str) -> Result<u64> {
    match operation.identifiers.as_slice() {
        [only] => Ok(*only),
        _ => Err(BridgeError::new("InvalidInput", &format!(
            "flatten cache failed validation: operation `{name}` needs exactly one identifier"))),
    }
}

/// `bool` → 「是否命中」。 / A `bool` as "did it hit".
fn one(hit: bool) -> bool { hit }

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol(identifier: u64, value: f64) -> FlattenSymbolSpec {
        FlattenSymbolSpec { identifier, value, dependencies: Vec::new() }
    }

    fn with_dependencies(identifier: u64, value: f64, dependencies: &[u64]) -> FlattenSymbolSpec {
        FlattenSymbolSpec { identifier, value, dependencies: dependencies.to_vec() }
    }

    fn op(name: &str, symbol: Option<FlattenSymbolSpec>) -> FlattenCacheOperation {
        FlattenCacheOperation { operation: name.to_owned(), symbol, identifiers: Vec::new() }
    }

    fn clearing(name: &str, identifiers: &[u64]) -> FlattenCacheOperation {
        FlattenCacheOperation { operation: name.to_owned(), symbol: None,
            identifiers: identifiers.to_vec() }
    }

    fn request(operations: Vec<FlattenCacheOperation>) -> FlattenCacheRequest {
        FlattenCacheRequest { operations }
    }

    #[test]
    fn flatten_cached_hits_without_flattening_and_force_overwrites() {
        let report = verify_flatten_cache(&request(vec![
            op("flattenCached", Some(symbol(1, 2.0))),
            // 命中：不再调用 `flatten`。 / A hit does not call `flatten` again.
            op("flattenCached", Some(symbol(1, 99.0))),
            // 强制：重新平展并覆盖缓存。 / Force re-flattens and overwrites.
            op("flattenForce", Some(symbol(1, 3.0))),
            // 现在缓存里是强制那次的值 3.0。 / The cache now holds the forced value 3.0.
            op("flattenCached", Some(symbol(1, 99.0))),
            op("cacheSize", None),
        ])).unwrap();
        assert_eq!(report.operations[0].value, Some(2.0));
        assert_eq!(report.operations[0].flattened, Some(true));
        assert_eq!(report.operations[0].polynomials_after, 1);
        // 第二次命中返回**缓存里的 2.0**，而不是新符号的 99.0——这就是"命中不发算"。 
        // The hit returns the **cached 2.0**, not the new symbol's 99.0.
        assert_eq!(report.operations[1].value, Some(2.0));
        assert_eq!(report.operations[1].flattened, Some(false));
        assert_eq!(report.operations[2].value, Some(3.0));
        assert_eq!(report.operations[2].flattened, Some(true));
        assert_eq!(report.operations[3].value, Some(3.0));
        assert_eq!(report.operations[3].flattened, Some(false));
        assert_eq!(report.operations[4].cache_size, Some(1));
        // 总调用次数 = 2（第一次与强制那次）。 / Two calls in total: the first and the forced one.
        assert_eq!(report.flatten_calls, 2);
        assert_eq!(report.final_cache_size, 1);
        assert!(!report.final_is_empty);
        assert_eq!(report.operation_vocabulary.len(), 11);
    }

    #[test]
    fn two_objects_sharing_an_identifier_share_one_cache_slot() {
        // `identifier()` 是键：两个不同符号只要标识符相同就共用一个槽，第二个拿到第一个的缓存值。
        // `identifier()` is the key: two different symbols sharing it share one slot and the second reads the first's value.
        let report = verify_flatten_cache(&request(vec![
            op("flattenCached", Some(symbol(7, 1.0))),
            op("flattenCached", Some(symbol(7, 2.0))),
            op("flattenCached", Some(symbol(8, 3.0))),
        ])).unwrap();
        assert_eq!(report.operations[0].value, Some(1.0));
        assert_eq!(report.operations[1].value, Some(1.0), "identifier 7 already cached by the first symbol");
        assert_eq!(report.operations[1].flattened, Some(false));
        assert_eq!(report.operations[2].value, Some(3.0));
        assert_eq!(report.operations[1].polynomials_after, 1);
        assert_eq!(report.operations[2].polynomials_after, 2);
        // 只平展了两次：标识符 7 一次、8 一次。 / Only two flattens: identifier 7 once, 8 once.
        assert_eq!(report.flatten_calls, 2);
    }

    #[test]
    fn clearing_one_cache_leaves_the_others_alone() {
        let report = verify_flatten_cache(&request(vec![
            op("flattenCached", Some(symbol(1, 1.0))),
            clearing("clearPolynomial", &[1]),
            // 再清一次：没命中。 / Clearing again misses.
            clearing("clearPolynomial", &[1]),
            op("cacheSize", None),
        ])).unwrap();
        // 清到了：`removed = true`，多项式缓存从 1 变 0。 / A hit reports `true` and the polynomial cache drops to 0.
        assert_eq!(report.operations[1].removed, Some(true));
        assert_eq!(report.operations[1].polynomials_after, 0);
        assert_eq!(report.operations[2].removed, Some(false));
        assert_eq!(report.operations[3].cache_size, Some(0));
        assert!(report.final_is_empty);
        // 清理不影响 `flatten` 计数。 / Clearing never changes the flatten count.
        assert_eq!(report.flatten_calls, 1);
    }

    #[test]
    fn dependency_checks_only_look_at_the_polynomial_cache() {
        // `a` 依赖 `b` 与 `c`；先把 `b` 平展进缓存。 / `a` depends on `b` and `c`; `b` is flattened first.
        let probe = with_dependencies(1, 10.0, &[2, 3]);
        let report = verify_flatten_cache(&request(vec![
            // 依赖都还不存在。 / Neither dependency exists yet.
            op("areDependenciesCached", Some(probe.clone())),
            op("flattenCached", Some(symbol(2, 20.0))),
            // 只满足一个：仍为假。 / Only one is satisfied: still false.
            op("areDependenciesCached", Some(probe.clone())),
            op("clearDependencyCaches", Some(probe.clone())),
            // 刚清掉 2（3 本来就没有）。 / This just cleared 2 (3 never existed).
            op("areDependenciesCached", Some(probe.clone())),
            op("clearItems", None),
            op("isCacheEmpty", None),
        ])).unwrap();
        assert_eq!(report.operations[0].flag, Some(false));
        assert_eq!(report.operations[2].flag, Some(false));
        assert_eq!(report.operations[2].polynomials_after, 1);
        // `clearDependencyCaches` 后什么都不剩。 / Nothing survives `clearDependencyCaches`.
        assert_eq!(report.operations[3].polynomials_after, 0);
        assert_eq!(report.operations[4].flag, Some(false));
        assert_eq!(report.operations[6].flag, Some(true));
        assert_eq!(report.flatten_calls, 1);
    }

    #[test]
    fn clear_items_and_clear_wipe_all_three_caches() {
        let mut report = verify_flatten_cache(&request(vec![
            op("flattenCached", Some(symbol(1, 1.0))),
            op("flattenCached", Some(symbol(2, 2.0))),
            clearing("clearItems", &[1]),
            op("cacheSize", None),
            clearing("clear", &[]),
            op("isCacheEmpty", None),
        ])).unwrap();
        assert_eq!(report.operations[2].polynomials_after, 1);
        assert_eq!(report.operations[3].cache_size, Some(1));
        assert_eq!(report.operations[5].flag, Some(true));
        assert_eq!(report.final_cache_size, 0);
        report.operations.clear();
        assert!(report.operations.is_empty());
    }

    #[test]
    fn bad_input_is_refused() {
        // 未知操作 / 缺符号 / 标识符个数不对。 / An unknown operation, a missing symbol, a wrong identifier count.
        // 已知操作但没给符号 → "needs a symbol"；未知操作 → "unknown operation"。两者必须分开报。
        // A known operation without a symbol says "needs a symbol"; an unknown one says "unknown operation".
        assert!(verify_flatten_cache(&request(vec![op("flattenForce", None)])).unwrap_err().message
            .contains("needs a symbol"));
        assert!(verify_flatten_cache(&request(vec![op("areDependenciesCached", None)])).unwrap_err().message
            .contains("needs a symbol"));
        let mut unknown = request(vec![op("flattenCached", Some(symbol(1, 1.0)))]);
        unknown.operations[0].operation = "peek".to_owned();
        assert!(verify_flatten_cache(&unknown).unwrap_err().message.contains("unknown operation"));
        assert!(verify_flatten_cache(&request(vec![op("flattenCached", None)])).unwrap_err().message
            .contains("needs a symbol"));
        // `clearPolynomial` 恰好要一个标识符。 / `clearPolynomial` needs exactly one identifier.
        assert!(verify_flatten_cache(&request(vec![clearing("clearPolynomial", &[1, 2])]))
            .unwrap_err().message.contains("exactly one identifier"));
        assert!(verify_flatten_cache(&request(vec![clearing("clearPolynomial", &[])]))
            .unwrap_err().message.contains("exactly one identifier"));
        // 上限。 / The cap.
        let many = request((0..=MAX_OPERATIONS).map(|_| op("cacheSize", None)).collect());
        assert!(verify_flatten_cache(&many).unwrap_err().message.contains("at most"));
        // 空请求：合法。 / A bare request is legal.
        let report = verify_flatten_cache(&request(Vec::new())).unwrap();
        assert_eq!(report.operation_count, 0);
        assert_eq!(report.final_cache_size, 0);
        assert!(report.final_is_empty);
    }
}

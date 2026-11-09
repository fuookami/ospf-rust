//! 续挖第 6 单：求值缓存与范围缓存的键身份及上下文语义（`model/value_cache.rs` + `model/range_cache.rs`）
//! Continuation, delivery 6: the key identity and context semantics of the value and range caches.
//!
//! 两个缓存长得几乎一样，但**键不一样**，这个不对称是这一单的核心：
//!
//! * `ValueCacheKey { kind, object_id }`——**三型 + 对象 ID**。所以同一个数字 ID 在 `polynomial` /
//!   `intermediateSymbol` / `objective` 三种身份下是**三个不同的键**，互不覆盖。
//! * `RangeCacheKey { object_id }`——**只有对象 ID**。所以范围缓存里**没有任何 kind 可以区分**：同一个数字 ID
//!   无论是多项式地址还是符号 ID，都落进同一个槽。这是两个缓存唯一实质性的差异，也是最容易被当成"对称"而写错的地方。
//!
//! 桥接不重写任何缓存逻辑：`get` / `set` / `get_or_compute` / `clear` / `clear_object` / `len` 全部是 trait 上的
//! **默认方法**，这一层只提供一个 `VecTokenList` 支撑的上下文实现并把操作序列回放一遍。两条只有读实现才知道的行为：
//!
//! 1. **`get_or_compute` 命中时不调用计算器**（`entry().or_insert_with`），所以重复取同一个键只会计算一次。
//! 2. **`clear_object` 返回布尔**——清到了 `true`，没清到 `false`；这是"删除是否命中"的唯一信号。
//!
//! The two caches look almost identical, but their **keys differ**, and that asymmetry is the core of this delivery:
//! `ValueCacheKey { kind, object_id }` carries **three kinds plus an object ID**, so one numeric ID under `polynomial` /
//! `intermediateSymbol` / `objective` is **three distinct keys** that never overwrite each other; while `RangeCacheKey {
//! object_id }` carries **only the ID**, so the range cache has **no kind to distinguish** — one numeric ID lands in one slot
//! whether it means a polynomial address or a symbol ID. That is the only substantive difference and the easiest thing to get
//! wrong by assuming symmetry. The bridge rewrites no cache logic: `get` / `set` / `get_or_compute` / `clear` /
//! `clear_object` / `len` are all trait **default methods**, and this layer only supplies a `VecTokenList`-backed context and
//! replays an operation log. Two behaviours are only visible in the implementation: **`get_or_compute` does not call the
//! computer on a hit** (`entry().or_insert_with`), and **`clear_object` returns a boolean** — the only signal of whether the
//! removal hit anything.

use std::collections::HashMap;
use std::fmt::Debug;

use ospf_rust_core::model::range_cache::{RangeCacheContextTrait, RangeCacheKey};
use ospf_rust_core::model::value_cache::{
    ValueCacheContextTrait, ValueCacheKey, ValueCacheKind,
};
use ospf_rust_core::token::VecTokenList;
use ospf_rust_core::variable::VariableRange;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个求值缓存键。 / One value-cache key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValueKeySpec {
    /// `polynomial` / `intermediateSymbol` / `objective`。 / The cache kind.
    pub kind: String,
    /// 对象 ID。 / The object ID.
    pub object_id: u64,
}

/// 一个范围缓存键。 / One range-cache key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RangeKeySpec {
    /// 对象 ID。 / The object ID.
    pub object_id: u64,
}

/// 一条回放操作。 / One replayed operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CacheOperationSpec {
    /// `set` / `get` / `getOrCompute` / `clear` / `clearObject` / `len`。 / The operation.
    pub operation: String,
    /// 求值缓存键（`range = false` 时用）。 / The value-cache key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value_key: Option<ValueKeySpec>,
    /// 范围缓存键（`range = true` 时用）。 / The range-cache key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub range_key: Option<RangeKeySpec>,
    /// `set` / `getOrCompute` 写入的值。 / The value written by `set` / `getOrCompute`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<f64>,
    /// 作用在哪一个缓存上。 / Which cache the operation targets.
    #[serde(default)]
    pub range: bool,
}

/// 回放请求。 / The replay request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CacheKeyIdentityRequest {
    /// 要比较身份的一组求值缓存键。 / The value-cache keys whose identity is compared.
    #[serde(default)]
    pub value_keys: Vec<ValueKeySpec>,
    /// 要比较身份的一组范围缓存键。 / The range-cache keys whose identity is compared.
    #[serde(default)]
    pub range_keys: Vec<RangeKeySpec>,
    /// 操作序列。 / The operation log.
    #[serde(default)]
    pub operations: Vec<CacheOperationSpec>,
}

/// 一条回放操作的结论。 / The verdict for one replayed operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CacheOperationRow {
    /// 操作名。 / The operation name.
    pub operation: String,
    /// 作用在哪个缓存上（`value` / `range`）。 / Which cache (`value` / `range`).
    pub cache: String,
    /// 返回的值（`get` / `getOrCompute` / `len` 之外为 `undefined`）。 / The returned value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub result: Option<f64>,
    /// `clearObject` 是否清到了东西。 / Whether `clearObject` hit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub removed: Option<bool>,
    /// 缓存当时的长度。 / The cache length at that moment.
    pub length: u32,
    /// `getOrCompute` 是否真的调用了计算器（命中时**不**调用）。 / Whether `getOrCompute` ran the computer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub computed: Option<bool>,
}

/// 键身份与回放结论。 / The key-identity and replay verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CacheKeyIdentityReport {
    /// 求值缓存键两两是否相等（按输入顺序的**上三角**，`i < j`）。 / Pairwise equality of the value keys.
    pub value_equal_pairs: Vec<Vec<bool>>,
    /// 范围缓存键两两是否相等。 / Pairwise equality of the range keys.
    pub range_equal_pairs: Vec<Vec<bool>>,
    /// 求值缓存里**去重后**的键数。 / The distinct value-key count.
    pub value_distinct: u32,
    /// 范围缓存里**去重后**的键数。 / The distinct range-key count.
    pub range_distinct: u32,
    /// 求值缓存键的身份是否包含 kind。 / Whether the value key's identity includes the kind.
    pub value_key_includes_kind: bool,
    /// 范围缓存键的身份是否包含 kind（**否**）。 / Whether the range key's identity includes a kind (**no**).
    pub range_key_includes_kind: bool,
    /// 同一 objectId 在不同 kind 下的键是否互不相等（桥接级一致性检查的结果）。 / Whether kinds keep keys apart.
    pub kinds_separate_value_keys: bool,
    /// 只靠 objectId 时两个缓存是否会把同一个 ID 混在一起。 / Whether one ID collides across the two caches.
    pub same_id_collides_in_range_cache: bool,
    /// 回放记录。 / The replay log.
    pub operations: Vec<CacheOperationRow>,
    /// 求值缓存种类词表（桥接定义）。 / The value-cache kind vocabulary (bridge-defined).
    pub kind_vocabulary: Vec<String>,
}

/// 操作上限（信任边界）。 / The operation cap (a trust boundary).
const MAX_OPERATIONS: usize = 4_096;

/// 求值缓存上下文：只做存储，行为全走 trait 默认方法。
/// The value-cache context: storage only, with all behaviour from the trait's default methods.
struct ValueContext {
    tokens: VecTokenList<f64>,
    cache: HashMap<ValueCacheKey, f64>,
    /// 计算器被调用的次数（用来断言 `get_or_compute` 命中时不计算）。用 `Arc<AtomicUsize>` 有两个原因：
    /// 闭包里不能再借 `self`（`get_or_compute` 拿走 `&mut self`），而这个上下文要实现 `Send + Sync`。
    /// How often the computer ran, to assert that a `get_or_compute` hit does not compute. `Arc<AtomicUsize>` for two
    /// reasons: the closure cannot borrow `self` again (the trait takes `&mut self`), and this context must be `Send + Sync`.
    computed: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl ValueCacheContextTrait<f64> for ValueContext {
    fn token_list(&self) -> &dyn ospf_rust_core::token::TokenList<f64> { &self.tokens }
    fn cache(&self) -> &HashMap<ValueCacheKey, f64> { &self.cache }
    fn cache_mut(&mut self) -> &mut HashMap<ValueCacheKey, f64> { &mut self.cache }
}

/// 范围缓存上下文。 / The range-cache context.
struct RangeContext {
    tokens: VecTokenList<f64>,
    cache: HashMap<RangeCacheKey, VariableRange<f64>>,
}

impl RangeCacheContextTrait<f64> for RangeContext {
    fn token_list(&self) -> &dyn ospf_rust_core::token::TokenList<f64> { &self.tokens }
    fn cache(&self) -> &HashMap<RangeCacheKey, VariableRange<f64>> { &self.cache }
    fn cache_mut(&mut self) -> &mut HashMap<RangeCacheKey, VariableRange<f64>> { &mut self.cache }
}

/// 解析求值缓存种类。 / Parse the value-cache kind.
fn parse_kind(text: &str) -> Result<ValueCacheKind> {
    match text {
        "polynomial" => Ok(ValueCacheKind::Polynomial),
        "intermediateSymbol" => Ok(ValueCacheKind::IntermediateSymbol),
        "objective" => Ok(ValueCacheKind::Objective),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "cache key identity failed validation: unknown cache kind `{other}`"))),
    }
}

/// 复核两个缓存的键身份并回放操作序列。非法输入是**错误**。
/// Re-check both caches' key identity and replay the operation log. Illegal input is an **error**.
pub fn verify_cache_key_identity(request: &CacheKeyIdentityRequest) -> Result<CacheKeyIdentityReport> {
    if request.operations.len() > MAX_OPERATIONS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "cache key identity failed validation: at most {MAX_OPERATIONS} operations are accepted")));
    }
    let value_keys = request.value_keys.iter()
        .map(|spec| Ok(ValueCacheKey { kind: parse_kind(&spec.kind)?, object_id: spec.object_id }))
        .collect::<Result<Vec<_>>>()?;
    let range_keys = request.range_keys.iter()
        .map(|spec| RangeCacheKey { object_id: spec.object_id })
        .collect::<Vec<_>>();
    let mut value_equal_pairs = Vec::new();
    for (index, left) in value_keys.iter().enumerate() {
        let mut row = Vec::new();
        for right in &value_keys[index + 1..] { row.push(left == right); }
        value_equal_pairs.push(row);
    }
    let mut range_equal_pairs = Vec::new();
    for (index, left) in range_keys.iter().enumerate() {
        let mut row = Vec::new();
        for right in &range_keys[index + 1..] { row.push(left == right); }
        range_equal_pairs.push(row);
    }
    // `ValueCacheKey` 不是 `Ord`，所以去重走 `HashSet`。 / `ValueCacheKey` is not `Ord`, so dedupe through a `HashSet`.
    let value_distinct_count = value_keys.iter().cloned()
        .collect::<std::collections::HashSet<_>>().len();
    let range_distinct_count = range_keys.iter().cloned()
        .collect::<std::collections::HashSet<_>>().len();
    // 同一 objectId 在三种 kind 下的键是否互不相等——这是"身份包含 kind"的直接后果。
    // Whether one ID under three kinds yields three distinct keys, the direct consequence of "identity includes the kind".
    let mut kinds_separate = true;
    let ids = value_keys.iter().map(|key| key.object_id)
        .collect::<std::collections::BTreeSet<_>>();
    for id in ids {
        let distinct = value_keys.iter().filter(|key| key.object_id == id).cloned()
            .collect::<std::collections::HashSet<_>>().len();
        let same_id = value_keys.iter().filter(|key| key.object_id == id).count();
        if distinct != same_id { kinds_separate = false; }
    }
    let (mut value_context, mut range_context) = (
        ValueContext { tokens: VecTokenList::new(), cache: HashMap::new(),
            computed: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)) },
        RangeContext { tokens: VecTokenList::new(), cache: HashMap::new() });
    let mut operations = Vec::with_capacity(request.operations.len());
    for spec in &request.operations {
        operations.push(replay(spec, &mut value_context, &mut range_context)?);
    }
    Ok(CacheKeyIdentityReport {
        value_equal_pairs,
        range_equal_pairs,
        value_distinct: u32::try_from(value_distinct_count).unwrap_or(u32::MAX),
        range_distinct: u32::try_from(range_distinct_count).unwrap_or(u32::MAX),
        value_key_includes_kind: true,
        range_key_includes_kind: false,
        kinds_separate_value_keys: kinds_separate,
        // 范围键只由 ID 构成，所以同一个 ID 必然同键。 / A range key is the ID alone, so one ID is always one key.
        same_id_collides_in_range_cache: range_keys.windows(2)
            .any(|pair| pair[0].object_id == pair[1].object_id),
        operations,
        kind_vocabulary: vec!["polynomial".to_owned(), "intermediateSymbol".to_owned(),
            "objective".to_owned()],
    })
}

/// 回放一条操作。 / Replay one operation.
fn replay(spec: &CacheOperationSpec, value: &mut ValueContext, range: &mut RangeContext)
    -> Result<CacheOperationRow>
{
    let name = spec.operation.as_str();
    let cache = if spec.range { "range" } else { "value" };
    // 每个操作都要对应的键；缺了就是调用方搞错了缓存。 / Every operation needs its key; a mismatch is a caller error.
    if spec.range && spec.range_key.is_none() {
        return Err(BridgeError::new("InvalidInput", &format!(
            "cache key identity failed validation: operation `{name}` on the range cache needs a range key")));
    }
    if !spec.range && spec.value_key.is_none() {
        return Err(BridgeError::new("InvalidInput", &format!(
            "cache key identity failed validation: operation `{name}` on the value cache needs a value key")));
    }
    let (result, removed, computed) = match (name, spec.range) {
        ("set", false) => {
            let key = value_key(spec)?;
            let written = spec.value.ok_or_else(|| BridgeError::new("InvalidInput",
                "cache key identity failed validation: `set` needs a value"))?;
            value.set(key, written);
            (None, None, None)
        }
        ("get", false) => (value.get(value_key(spec)?).copied(), None, None),
        ("getOrCompute", false) => {
            let key = value_key(spec)?;
            let produced = spec.value.ok_or_else(|| BridgeError::new("InvalidInput",
                "cache key identity failed validation: `getOrCompute` needs a value"))?;
            // 计算器只在**未命中**时被调用：计数器增减就是证据。计数器先克隆出来，避免与 `&mut self` 冲突。
            // The computer only runs on a miss, and the counter is cloned out first to avoid fighting `&mut self`.
            let counter = std::sync::Arc::clone(&value.computed);
            let before = counter.load(std::sync::atomic::Ordering::Relaxed);
            let got = *value.get_or_compute(key, || {
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                produced
            });
            let after = counter.load(std::sync::atomic::Ordering::Relaxed);
            (Some(got), None, Some(after != before))
        }
        ("clear", false) => { value.clear(); (None, None, None) }
        ("clearObject", false) => (None, Some(value.clear_object(value_key(spec)?)), None),
        ("len", false) => (Some(value.len() as f64), None, None),
        ("set", true) => {
            let key = range_key(spec)?;
            let written = spec.value.ok_or_else(|| BridgeError::new("InvalidInput",
                "cache key identity failed validation: `set` needs a value"))?;
            // 范围缓存里没有 kind，只有一个 ID；用 `[value, value]` 作为退化区间即可。
            // The range cache holds no kind, only an ID, so a degenerate `[value, value]` interval suffices.
            range.set(key, VariableRange::new(Some(written), Some(written)));
            (None, None, None)
        }
        ("get", true) => (range.get(range_key(spec)?).and_then(|range| range.lower_bound), None, None),
        ("getOrCompute", true) => {
            let key = range_key(spec)?;
            let produced = spec.value.ok_or_else(|| BridgeError::new("InvalidInput",
                "cache key identity failed validation: `getOrCompute` needs a value"))?;
            let hit = range.cache.contains_key(&key);
            let got = range.get_or_compute(key, || VariableRange::new(Some(produced), Some(produced)));
            (got.lower_bound, None, Some(!hit))
        }
        ("clear", true) => { range.clear(); (None, None, None) }
        ("clearObject", true) => (None, Some(range.clear_object(range_key(spec)?)), None),
        ("len", true) => (Some(range.len() as f64), None, None),
        (other, _) => return Err(BridgeError::new("InvalidInput", &format!(
            "cache key identity failed validation: unknown operation `{other}`"))),
    };
    let length = if spec.range { range.len() } else { value.len() };
    Ok(CacheOperationRow { operation: name.to_owned(), cache: cache.to_owned(), result, removed,
        length: u32::try_from(length).unwrap_or(u32::MAX), computed })
}

/// 从操作里取求值缓存键。 / Take the value-cache key out of an operation.
fn value_key(spec: &CacheOperationSpec) -> Result<ValueCacheKey> {
    let key = spec.value_key.as_ref().expect("checked by the caller");
    Ok(ValueCacheKey { kind: parse_kind(&key.kind)?, object_id: key.object_id })
}

/// 从操作里取范围缓存键。 / Take the range-cache key out of an operation.
fn range_key(spec: &CacheOperationSpec) -> Result<RangeCacheKey> {
    Ok(RangeCacheKey { object_id: spec.range_key.as_ref().expect("checked by the caller").object_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_key(kind: &str, object_id: u64) -> ValueKeySpec {
        ValueKeySpec { kind: kind.to_owned(), object_id }
    }

    fn operation(name: &str, range: bool) -> CacheOperationSpec {
        CacheOperationSpec { operation: name.to_owned(), value_key: None, range_key: None,
            value: None, range }
    }

    fn value_op(name: &str, kind: &str, object_id: u64, value: Option<f64>) -> CacheOperationSpec {
        CacheOperationSpec { operation: name.to_owned(),
            value_key: Some(value_key(kind, object_id)), range_key: None, value, range: false }
    }

    fn range_op(name: &str, object_id: u64, value: Option<f64>) -> CacheOperationSpec {
        CacheOperationSpec { operation: name.to_owned(), value_key: None,
            range_key: Some(RangeKeySpec { object_id }), value, range: true }
    }

    fn request() -> CacheKeyIdentityRequest {
        CacheKeyIdentityRequest { value_keys: Vec::new(), range_keys: Vec::new(),
            operations: Vec::new() }
    }

    #[test]
    fn the_value_key_carries_a_kind_while_the_range_key_does_not() {
        let mut document = request();
        // 同一个 ID、三种 kind：三个不同的键。 / One ID under three kinds: three distinct keys.
        document.value_keys = vec![value_key("polynomial", 7),
            value_key("intermediateSymbol", 7), value_key("objective", 7)];
        // 范围键只有 ID：同一个 ID 两次就是同一个键。 / A range key is the ID alone, so two of them are one key.
        document.range_keys = vec![RangeKeySpec { object_id: 7 }, RangeKeySpec { object_id: 7 }];
        let report = verify_cache_key_identity(&document).unwrap();
        assert_eq!(report.value_distinct, 3);
        assert!(report.kinds_separate_value_keys);
        assert!(report.value_key_includes_kind);
        // 上三角全 false：三种 kind 互不相等。 / The upper triangle is all false: no two kinds are equal.
        assert_eq!(report.value_equal_pairs, vec![vec![false, false], vec![false], vec![]]);
        assert_eq!(report.range_distinct, 1);
        assert!(!report.range_key_includes_kind);
        assert_eq!(report.range_equal_pairs, vec![vec![true], vec![]]);
        assert!(report.same_id_collides_in_range_cache);
        assert_eq!(report.kind_vocabulary.len(), 3);
        // 同一 kind 同一 ID 才相等。 / Only the same kind and ID are equal.
        let mut same = request();
        same.value_keys = vec![value_key("objective", 7), value_key("objective", 7),
            value_key("objective", 8)];
        let report = verify_cache_key_identity(&same).unwrap();
        assert_eq!(report.value_equal_pairs, vec![vec![true, false], vec![false], vec![]]);
        assert_eq!(report.value_distinct, 2);
        assert!(report.operations.is_empty(), "no operations were requested");
    }

    #[test]
    fn get_or_compute_computes_once_and_clear_object_reports_whether_it_hit() {
        let mut document = request();
        document.operations = vec![
            value_op("set", "polynomial", 1, Some(2.5)),
            value_op("get", "polynomial", 1, None),
            // 命中：计算器**不**调用，值仍是 2.5。 / A hit never runs the computer; the value stays 2.5.
            value_op("getOrCompute", "polynomial", 1, Some(99.0)),
            // 未命中：用传入的值。 / A miss takes the supplied value.
            value_op("getOrCompute", "objective", 1, Some(4.5)),
            // 同一个 ID 不同 kind → 另一个槽。 / Same ID, another kind → another slot.
            value_op("get", "objective", 1, None),
            value_op("len", "objective", 0, None),
            // 清掉一个存在的键：`removed = true`。 / Clearing an existing key reports `true`.
            value_op("clearObject", "polynomial", 1, None),
            value_op("len", "objective", 0, None),
            // 再清一次：`removed = false`。 / Clearing again reports `false`.
            value_op("clearObject", "polynomial", 1, None),
            value_op("get", "polynomial", 1, None),
            value_op("clear", "objective", 0, None),
            value_op("len", "objective", 0, None),
        ];
        let report = verify_cache_key_identity(&document).unwrap();
        let row = |index: usize| report.operations[index].clone();
        assert_eq!(row(0).result, None);
        assert_eq!(row(0).length, 1);
        assert_eq!(row(1).result, Some(2.5));
        // 命中时 `computed = false`，且值没被新值覆盖。 / On a hit `computed` is false and the value is unchanged.
        assert_eq!(row(2).result, Some(2.5));
        assert_eq!(row(2).computed, Some(false));
        // 未命中时 `computed = true`。 / A miss computes.
        assert_eq!(row(3).result, Some(4.5));
        assert_eq!(row(3).computed, Some(true));
        assert_eq!(row(3).length, 2);
        assert_eq!(row(4).result, Some(4.5));
        assert_eq!(row(5).result, Some(2.0));
        assert_eq!(row(6).removed, Some(true));
        assert_eq!(row(6).length, 1);
        assert_eq!(row(7).result, Some(1.0));
        assert_eq!(row(8).removed, Some(false));
        assert_eq!(row(9).result, None);
        assert_eq!(row(10).length, 0);
        assert_eq!(row(11).result, Some(0.0));
        assert_eq!(row(0).cache, "value");
    }

    #[test]
    fn the_range_cache_has_no_kind_so_one_id_is_one_slot() {
        let mut document = request();
        document.operations = vec![
            range_op("set", 3, Some(1.5)),
            // 同一个数字 ID 换一种"含义"也还是同一个槽——这正是它与求值缓存的差别。
            // The same numeric ID keeps one slot no matter what it "means" — the very difference from the value cache.
            range_op("get", 3, None),
            range_op("getOrCompute", 3, Some(9.0)),
            range_op("len", 3, None),
            range_op("clearObject", 3, None),
            range_op("len", 3, None),
        ];
        let report = verify_cache_key_identity(&document).unwrap();
        assert_eq!(report.operations[0].cache, "range");
        assert_eq!(report.operations[1].result, Some(1.5));
        assert_eq!(report.operations[2].result, Some(1.5));
        assert_eq!(report.operations[2].computed, Some(false));
        assert_eq!(report.operations[3].result, Some(1.0));
        assert_eq!(report.operations[4].removed, Some(true));
        assert_eq!(report.operations[5].result, Some(0.0));
    }

    #[test]
    fn bad_input_is_refused() {
        // 未知 kind / 未知操作 / 缺键 / 缺值。 / An unknown kind, an unknown operation, a missing key, a missing value.
        let mut bad_kind = request();
        bad_kind.value_keys = vec![value_key("range", 1)];
        assert!(verify_cache_key_identity(&bad_kind).unwrap_err().message.contains("unknown cache kind"));
        let mut bad_op = request();
        bad_op.operations = vec![operation("peek", false)];
        assert!(verify_cache_key_identity(&bad_op).unwrap_err().message.contains("needs a value key"));
        let mut unknown = request();
        let mut misspecified = value_op("peek", "polynomial", 1, None);
        misspecified.operation = "peek".to_owned();
        unknown.operations = vec![misspecified];
        assert!(verify_cache_key_identity(&unknown).unwrap_err().message.contains("unknown operation"));
        // 给范围缓存一个只有求值键的操作。 / An operation with only a value key aimed at the range cache.
        let mut mismatched = request();
        let mut spec = value_op("set", "polynomial", 1, Some(1.0));
        spec.range = true;
        mismatched.operations = vec![spec];
        assert!(verify_cache_key_identity(&mismatched).unwrap_err().message.contains("needs a range key"));
        // `set` 缺值。 / `set` without a value.
        let mut no_value = request();
        no_value.operations = vec![value_op("set", "polynomial", 1, None)];
        assert!(verify_cache_key_identity(&no_value).unwrap_err().message.contains("needs a value"));
        // 上限。 / The cap.
        let mut too_many = request();
        too_many.operations = (0..=MAX_OPERATIONS)
            .map(|index| value_op("get", "polynomial", index as u64, None)).collect();
        assert!(verify_cache_key_identity(&too_many).unwrap_err().message.contains("at most"));
        // 空请求：合法，全空。 / A bare request is legal and empty.
        let report = verify_cache_key_identity(&request()).unwrap();
        assert_eq!(report.value_distinct, 0);
        assert_eq!(report.range_distinct, 0);
        assert!(report.operations.is_empty());
        assert!(!report.same_id_collides_in_range_cache);
    }
}

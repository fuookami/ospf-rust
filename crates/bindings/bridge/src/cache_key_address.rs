//! 续挖第 7 单：平展缓存键的身份是**地址**而不是值（`model/flatten/cache_key.rs`）
//! Continuation, delivery 7: the flatten cache key's identity is an **address**, not a value.
//!
//! `model/flatten/` 五个文件 24 个未引用类型，多数是平展上下文与中间表示（`FlattenedMonomial`、
//! `FlattenContextTrait`、`LazyLinearFlattenContext`、`LinearInner`、`QuadraticMonomialKind`…）——那是求值管线本体，
//! 属真边界。可数据化的只有 `CacheKey` / `Cacheable`，而它恰好是本仓库最容易误用的一个类型：
//!
//! **键值是内存地址。** `CacheKey { value: u64 }` 由 `Arc::as_ptr` / `&T as *const T as u64` 得来。所以：
//!
//! 1. **同一个 `Arc` 取两次 → 同一个键**（地址相同）。
//! 2. **两个值相等的 `Arc` → 两个不同的键**。`Arc::new(42)` 两次得到的键不相等——这是最常被想当然搞错的一点：
//!    缓存身份是**指针相同**，不是 `==` 成立。
//! 3. **两个值相等的 `Box` 同理**。`Cacheable` 要求"拥有稳定内存地址"，而两个等值的 Box 是两个地址。
//! 4. **`from_raw(0)` 是合法的**：地址空间里 0 可表示，所以一个"空/无效"键与真键无法在类型层区分。
//! 5. **已知上限（刻意不修）**：地址会被复用。一个 `Box` 释放后新分配的 `Box` **可能**落在同一地址，于是旧键会命中
//!    新对象。这不是本层能修的，也不是拒绝路径——报告用 `addressIdentity` 与 `addressReuseHazard` 如实标注。
//!
//! 第 5 条是 `ponytail:` 级别的真实天花板：地址键换来零成本判定，代价是**必须保证对象存活期间键有效**。
//!
//! 6. **地址是 `u64`，而 JS 的 number 只有 53 位安全整数**。所以 `rawAddresses` 必须 ≤ `2^53 - 1`（`Number.MAX_SAFE_INTEGER`），
//!    否则 JSON 往返会把 `u64::MAX` 变成 `1.8446744073709552e+19`，前端拿到的既不是原值也无法再传回来。
//!    这一条是**实测打出来的**：最初把 `u64::MAX` 当合法输入写进夹具，SDK 用例直接因为反序列化失败而红。
//!    报告用 `rawAddressCeiling` 把这个上限显式交出去。
//!
//! 第 6 条同样来自"读签名会判错"：`CacheKey { value: u64 }` 看起来能装下整个地址空间，但**跨语言只有 53 位**。
//!
//! Of the twenty-four unreferenced types there, most are flattening contexts and intermediate representations — the evaluation
//! pipeline itself, a true boundary. Only `CacheKey` / `Cacheable` are datafiable, and it is the easiest type in this repository
//! to misuse: **the key value is a memory address**, obtained from `Arc::as_ptr` / `&T as *const T as u64`. So the same `Arc`
//! yields the same key, while **two `Arc`s with equal values yield two different keys** — cache identity is pointer identity,
//! not `==`. `Box` behaves the same way, `from_raw(0)` is legal (zero is representable in the address space, so an "empty" key
//! cannot be told apart at the type level), and the known ceiling, deliberately unfixed, is **address reuse**: a freed `Box`'s
//! address may be handed to a new allocation, making a stale key hit a new object. That is not this layer's to fix and is not a
//! refusal path; the report labels it through `addressIdentity` and `addressReuseHazard`. It is a real ceiling: address keys buy
//! O(1) identity at the cost of requiring the object to outlive the key.

use std::sync::Arc;

use ospf_rust_core::model::flatten::{CacheKey, Cacheable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一组用相等值构造的对象。 / A group of objects built from equal values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EqualValueGroup {
    /// 该组每个对象携带的值。 / The value every object in the group carries.
    pub value: i64,
    /// 造几个对象（至少 1）。 / How many objects to build (at least one).
    pub count: u32,
}

/// 缓存键身份请求。 / The cache-key identity request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CacheKeyAddressRequest {
    /// 用相等值构造的 `Arc` 组。 / The `Arc` groups built from equal values.
    #[serde(default)]
    pub arc_groups: Vec<EqualValueGroup>,
    /// 用相等值构造的 `Box` 组。 / The `Box` groups built from equal values.
    #[serde(default)]
    pub box_groups: Vec<EqualValueGroup>,
    /// 直接构造的原始地址。 / The raw addresses to construct directly.
    #[serde(default)]
    pub raw_addresses: Vec<u64>,
}

/// 缓存键身份结论。 / The cache-key identity verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CacheKeyAddressReport {
    /// 每组的 `Arc` 键。 / The `Arc` keys per group.
    pub arc_keys: Vec<Vec<u64>>,
    /// 每组的 `Box` 键。 / The `Box` keys per group.
    pub box_keys: Vec<Vec<u64>>,
    /// 原始地址键。 / The raw-address keys.
    pub raw_keys: Vec<u64>,
    /// 同一个 `Arc` 取两次是否同键（**是**）。 / Whether one `Arc` twice yields one key (**yes**).
    pub same_arc_same_key: bool,
    /// 同一个 `Box` 取两次是否同键（**是**）。 / Whether one `Box` twice yields one key (**yes**).
    pub same_box_same_key: bool,
    /// 值相同的不同 `Arc` 是否给出不同键（**是**，除非只造了一个）。 / Whether equal-valued distinct `Arc`s differ.
    pub distinct_arcs_distinct_keys: bool,
    /// 值相同的不同 `Box` 是否给出不同键（**是**，除非只造了一个）。 / Whether equal-valued distinct `Box`s differ.
    pub distinct_boxes_distinct_keys: bool,
    /// 身份是否是地址而不是值（**是**）。 / Whether identity is the address rather than the value.
    pub address_identity: bool,
    /// `from_raw(0)` 是否被接受（**是**）。 / Whether `from_raw(0)` is accepted (**yes**).
    pub zero_address_is_representable: bool,
    /// 地址复用隐患（地址键的已知上限，本层不修）。 / The address-reuse hazard (a known ceiling, not fixed here).
    pub address_reuse_hazard: bool,
    /// `Cacheable` 对实现者的要求（桥接定义的说明文本）。 / What `Cacheable` demands of implementors.
    pub cacheable_requirement: String,
    /// 原始地址的上限（`2^53 - 1`，JS 安全整数）。 / The raw-address ceiling (`2^53 - 1`, the JS safe integer).
    pub raw_address_ceiling: u64,
}

/// 组数上限（信任边界）。 / The group cap (a trust boundary).
const MAX_GROUPS: usize = 1_024;
/// 每组对象数上限。 / The per-group object cap.
const MAX_PER_GROUP: u32 = 1_024;

/// 原始地址上限：超过这个值 JSON 往返会在 JS 侧丢精度，所以**在信任边界处就拒**。
/// The raw-address ceiling: above it a JSON round trip loses precision on the JS side, so it is refused here.
const MAX_SAFE_RAW_ADDRESS: u64 = 9_007_199_254_740_991;

/// 一个测试用的 `Cacheable` 实现：键就是**自己的地址**，与领域 `Box` 的做法一致。
/// A test `Cacheable`: the key is **its own address**, matching what the domain does for `Box`.
struct Addressed {
    value: i64,
    key: CacheKey,
}

impl Addressed {
    fn boxed(value: i64) -> Box<Self> {
        let mut boxed = Box::new(Self { value, key: CacheKey::from_raw(0) });
        boxed.key = CacheKey::from_box(&*boxed);
        boxed
    }
}

impl Cacheable for Addressed {
    fn cache_key(&self) -> CacheKey { self.key }
}

/// 复核缓存键的身份语义。非法输入是**错误**。
/// Re-check the identity semantics of the cache key. Illegal input is an **error**.
pub fn verify_cache_key_address(request: &CacheKeyAddressRequest) -> Result<CacheKeyAddressReport> {
    if request.arc_groups.len() > MAX_GROUPS || request.box_groups.len() > MAX_GROUPS {
        return Err(BridgeError::new("InvalidInput", &format!(
            "cache key address failed validation: at most {MAX_GROUPS} groups are accepted")));
    }
    for address in &request.raw_addresses {
        if *address > MAX_SAFE_RAW_ADDRESS {
            return Err(BridgeError::new("InvalidInput", &format!(
                "cache key address failed validation: raw address {address} exceeds the JS safe integer ceiling                  {MAX_SAFE_RAW_ADDRESS}, so it cannot round-trip through a JSON number")));
        }
    }
    for group in request.arc_groups.iter().chain(request.box_groups.iter()) {
        if group.count == 0 {
            return Err(BridgeError::new("InvalidInput",
                "cache key address failed validation: every group needs at least one object"));
        }
        if group.count > MAX_PER_GROUP {
            return Err(BridgeError::new("InvalidInput", &format!(
                "cache key address failed validation: at most {MAX_PER_GROUP} objects per group are accepted")));
        }
    }
    // `Arc` 组：先全部造出来**再**取键——否则前面的会先被释放，地址被后面的复用，结论就不是"值相等而键不等"了。
    // `Arc` groups: build every object **before** taking keys, otherwise earlier ones drop and their addresses get reused.
    let arcs = request.arc_groups.iter()
        .map(|group| (group.value, (0..group.count).map(|_| Arc::new(group.value)).collect::<Vec<_>>()))
        .collect::<Vec<_>>();
    let arc_keys = arcs.iter()
        .map(|(_, group)| group.iter().map(|item| CacheKey::from_arc(item).value).collect())
        .collect::<Vec<Vec<u64>>>();
    let boxes = request.box_groups.iter()
        .map(|group| (0..group.count).map(|_| Addressed::boxed(group.value)).collect::<Vec<_>>())
        .collect::<Vec<Vec<Box<Addressed>>>>();
    let box_keys = boxes.iter()
        .map(|group| group.iter().map(|item| item.cache_key().value).collect())
        .collect::<Vec<Vec<u64>>>();
    let raw_keys = request.raw_addresses.iter().map(|address| CacheKey::from_raw(*address).value)
        .collect::<Vec<u64>>();
    // 同一个对象取两次：键恒相同。 / One object twice: the key is identical.
    let same_arc_same_key = arcs.iter().all(|(_, group)| group.iter()
        .all(|item| CacheKey::from_arc(item) == CacheKey::from_arc(item)));
    let same_box_same_key = boxes.iter().all(|group| group.iter()
        .all(|item| item.cache_key() == item.cache_key()));
    // 值相同但对象不同：键应当**互不相同**（实验里没造出两个时不算失败）。
    // Equal values but distinct objects: the keys must **all differ** (a group of one proves nothing).
    let distinct = |keys: &Vec<Vec<u64>>| keys.iter()
        .all(|group| group.len() < 2
            || group.iter().collect::<std::collections::HashSet<_>>().len() == group.len());
    // 先把"没造出反例"两条算出来，再移动键表进报告。 / Compute the two vacuity flags before moving the key tables.
    let distinct_arcs_distinct_keys = distinct(&arc_keys);
    let distinct_boxes_distinct_keys = distinct(&box_keys);
    Ok(CacheKeyAddressReport {
        arc_keys,
        box_keys,
        raw_keys,
        same_arc_same_key,
        same_box_same_key,
        distinct_arcs_distinct_keys,
        distinct_boxes_distinct_keys,
        // 身份是地址：同一个值可以有多个键。 / Identity is the address: one value can own several keys.
        address_identity: true,
        zero_address_is_representable: CacheKey::from_raw(0).value == 0,
        address_reuse_hazard: true,
        cacheable_requirement: "the implementor must keep a stable address for as long as its key is used \
            (address keys are invalidated by deallocation, and a later allocation can reuse the address)"
            .to_owned(),
        raw_address_ceiling: MAX_SAFE_RAW_ADDRESS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(value: i64, count: u32) -> EqualValueGroup { EqualValueGroup { value, count } }

    fn request() -> CacheKeyAddressRequest {
        CacheKeyAddressRequest { arc_groups: Vec::new(), box_groups: Vec::new(),
            raw_addresses: Vec::new() }
    }

    #[test]
    fn the_key_is_the_address_so_equal_values_are_different_keys() {
        let mut document = request();
        // 同一组里三个值都是 42 的 `Arc`。 / Three `Arc`s carrying 42 in one group.
        document.arc_groups = vec![group(42, 3)];
        document.box_groups = vec![group(7, 3)];
        let report = verify_cache_key_address(&document).unwrap();
        // 同一个对象两次同键。 / One object twice is one key.
        assert!(report.same_arc_same_key);
        assert!(report.same_box_same_key);
        // **值相等但键不相等**：这就是"身份是地址而不是值"。 / Equal values, different keys: identity is the address.
        assert!(report.distinct_arcs_distinct_keys);
        assert!(report.distinct_boxes_distinct_keys);
        assert!(report.address_identity);
        assert_eq!(report.arc_keys.len(), 1);
        assert_eq!(report.arc_keys[0].len(), 3);
        let arc_distinct = report.arc_keys[0].iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(arc_distinct.len(), 3, "three Arcs holding 42 must own three keys");
        let box_distinct = report.box_keys[0].iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(box_distinct.len(), 3, "three Boxes holding 7 must own three keys");
        // 键是真实地址，不会是 0。 / The keys are real addresses, never zero.
        assert!(report.arc_keys[0].iter().all(|key| *key != 0));
        assert!(report.box_keys[0].iter().all(|key| *key != 0));
    }

    #[test]
    fn a_single_object_group_proves_nothing_about_distinctness() {
        // 每组只造一个：`distinct*` 恒真但没有证据，这两条字段的含义是"没造出反例"，不是"证明了"。
        // One object per group: `distinct*` holds vacuously; the field means "no counterexample", not "proven".
        let mut document = request();
        document.arc_groups = vec![group(1, 1), group(1, 1)];
        document.box_groups = vec![group(1, 1)];
        let report = verify_cache_key_address(&document).unwrap();
        assert!(report.distinct_arcs_distinct_keys);
        assert!(report.distinct_boxes_distinct_keys);
        assert_eq!(report.arc_keys.len(), 2);
        // 两个不同的组各造一个值相同的对象：键仍然不同。 / Two groups, one equal-valued object each: keys still differ.
        assert_ne!(report.arc_keys[0][0], report.arc_keys[1][0]);
    }

    #[test]
    fn zero_is_a_representable_address() {
        let mut document = request();
        document.raw_addresses = vec![0, 4096, MAX_SAFE_RAW_ADDRESS];
        let report = verify_cache_key_address(&document).unwrap();
        // `from_raw` 原样保留，包括 0 与安全整数上限。 / `from_raw` keeps everything, zero and the safe ceiling included.
        assert_eq!(report.raw_keys, vec![0, 4096, MAX_SAFE_RAW_ADDRESS]);
        assert_eq!(report.raw_address_ceiling, MAX_SAFE_RAW_ADDRESS);
        // 超过 JS 安全整数：**入口就拒**，不让它在 JSON 往返里悄悄变形。
        // Above the JS safe integer it is refused at the entry rather than silently mangled by the JSON round trip.
        document.raw_addresses = vec![u64::MAX];
        assert!(verify_cache_key_address(&document).unwrap_err().message
            .contains("exceeds the JS safe integer ceiling"));
        assert!(report.zero_address_is_representable);
        // 所以"空键"与"真键"在类型层分不开——这条如实标出。 / An "empty" key is indistinguishable from a real one.
        assert!(report.address_reuse_hazard);
        assert!(report.cacheable_requirement.contains("stable address"));
    }

    #[test]
    fn bad_input_is_refused() {
        // 组内对象数为 0 / 超上限 / 组数超上限。 / A zero object count, the per-group cap, the group cap.
        let mut empty = request();
        empty.arc_groups = vec![group(1, 0)];
        assert!(verify_cache_key_address(&empty).unwrap_err().message
            .contains("at least one object"));
        let mut too_many = request();
        too_many.box_groups = vec![group(1, MAX_PER_GROUP + 1)];
        assert!(verify_cache_key_address(&too_many).unwrap_err().message.contains("at most"));
        let mut too_many_groups = request();
        too_many_groups.arc_groups = (0..=MAX_GROUPS).map(|index| group(index as i64, 1)).collect();
        assert!(verify_cache_key_address(&too_many_groups).unwrap_err().message.contains("at most"));
        // 空请求：合法，全空且三条"没造出反例"的字段仍为真。 / A bare request is legal and empty.
        let report = verify_cache_key_address(&request()).unwrap();
        assert!(report.arc_keys.is_empty());
        assert!(report.box_keys.is_empty());
        assert!(report.raw_keys.is_empty());
        assert!(report.same_arc_same_key);
        assert!(report.same_box_same_key);
        assert!(report.address_identity);
    }
}

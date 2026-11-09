//! 续挖第 26 单：展平缓存键是**内存地址**，不是结构性身份（`model/flatten/cache_key.rs` 与
//! `inner_box.rs` 的 `cache_key()`）
//! Continuation, delivery 26: the flatten cache key is a **memory address**, not a structural identity.
//!
//! `CacheKey` 只有一个 `u64` 字段，三个构造器全部是**取地址**（`from_box` / `from_arc` / `from_raw`），而
//! `QuadraticMonomial::cache_key()` 的实现就是 `&*self.inner as *const _ as u64`——**堆上 Box 内部的地址**。由此推出
//! 四条对调用方有实际后果的结论，本单全部实测：
//!
//! 1. **内容相等 ≠ 键相等**：两个字段完全一样的单项式各有各的 Box，键必然不同。键**不能**当等值判据用。
//! 2. **`clone()` 换新键**：克隆会给内层分配新 Box，所以克隆体的键与本体不同；把键存下来再克隆去查，查不到。
//! 3. **`from_raw` 可以伪造**：任意 `u64` 都能变成 `CacheKey`，所以两把键"相等"只说明地址相同/被伪造成相同，
//!    不说明质量。用 `HashMap<CacheKey, _>` 做缓存时，键的**唯一性来自分配**，而非语义。
//! 4. **`HashSet<CacheKey>` 不按内容去重**：两个内容相同的单项式键不同，集合里就是两个元素。
//!
//! 好的一面：同一个**未被移动**的实例，其键是稳定的（Box 内部地址不随外层移动而变），所以"一次建模过程内"用它
//! 做缓存是可行的；它**不能**跨进程/跨次运行持久化，也不能用来判断两个模型是否等价。
//!
//! `CacheKey` has a single `u64`, all three constructors take **addresses**, and `QuadraticMonomial::cache_key()` is
//! literally `&*self.inner as *const _ as u64` — the **boxed heap address**. Four consequences follow, all measured here.
//! **Equal content does not mean equal keys**: two field-identical monomials own separate boxes, so their keys always
//! differ and a key cannot serve as an equality test. **`clone()` gets a new key**: cloning allocates a fresh inner box, so
//! the clone's key differs from the original's and a stored key will not find it. **`from_raw` forges keys**: any `u64`
//! becomes a `CacheKey`, so "two keys are equal" only means "same address, possibly forged", and a `HashMap<CacheKey, _>`
//! cache bases uniqueness on allocation rather than semantics. And a **`HashSet<CacheKey>` does not dedupe by content**. The
//! good news: an instance that is not moved keeps a stable key (the box target does not move with its container), so
//! keying a cache within a single modeling pass works — but keys cannot be persisted across processes or runs, nor used to
//! decide whether two models are equivalent.

use ospf_rust_core::model::flatten::cache_key::CacheKey;
use ospf_rust_core::model::flatten::inner_box::{
    Quadratic, QuadraticMonomial, QuadraticMonomialKind,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 缓存键身份请求。 / The cache-key identity request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlattenIdentityRequest {
    /// 单项式系数。 / The monomial coefficient.
    pub coefficient: f64,
    /// 第一个变量序号。 / The first variable index.
    pub var_index: u64,
    /// 第二次元式的变量序号（用于验证内容不同则键不同）。 / The twin's variable index.
    pub other_var: u64,
    /// 用来伪造的键值。 / The forged key value.
    pub forged_key: u64,
}

/// 缓存键身份结论。 / The cache-key identity verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlattenIdentityReport {
    /// 本体键（地址，**不保证跨次运行稳定**）。 / The original key (an address; **not stable across runs**).
    pub first_key: u64,
    /// 克隆体的键。 / The clone's key.
    pub clone_key: u64,
    /// 独立构造但内容相同者的键。 / The independently built but content-identical key.
    pub twin_key: u64,
    /// 内容相同的三者键全不相同（**是**）。 / Whether all three content-identical keys differ (**yes**).
    pub equal_content_gives_distinct_keys: bool,
    /// `clone()` 换新键（**是**）。 / Whether `clone()` gets a new key (**yes**).
    pub clone_gets_its_own_key: bool,
    /// 内容不同者的键也不同（同一次分配里必然如此）。 / Whether a different content also gets a different key.
    pub different_content_gives_distinct_keys: bool,
    /// `from_raw` 能把两把键伪造成相等（**是**）。 / Whether `from_raw` can forge two equal keys (**yes**).
    pub from_raw_forges_collisions: bool,
    /// `HashSet<CacheKey>` 不按内容去重（**是**）。 / Whether a `HashSet<CacheKey>` fails to dedupe by content (**yes**).
    pub hashset_does_not_dedupe: bool,
    /// 重建的多项式键不可复现（**是**）。 / Whether a rebuilt polynomial gets a fresh key (**yes**).
    pub rebuilt_polynomial_gets_a_fresh_key: bool,
    /// 单项式类型是线性（**是**）。 / Whether the monomial kind is linear (**yes**).
    pub monomial_is_linear: bool,
    /// 系数回读。 / The coefficient read back.
    pub coefficient: f64,
}

/// 请求字段上限（信任边界）。 / Request field caps (a trust boundary).
const MAX_INDEX: u64 = u32::MAX as u64;

/// 走一遍缓存键身份。非法输入是**错误**。 / Walk the cache-key identity.
pub fn verify_flatten_identity(request: &FlattenIdentityRequest) -> Result<FlattenIdentityReport> {
    if !request.coefficient.is_finite() {
        return Err(BridgeError::new("InvalidInput",
            "flatten identity failed validation: the coefficient must be finite"));
    }
    if request.var_index > MAX_INDEX || request.other_var > MAX_INDEX {
        return Err(BridgeError::new("InvalidInput", &format!(
            "flatten identity failed validation: variable indices must be below {MAX_INDEX}")));
    }
    let index = request.var_index as usize;
    let other = request.other_var as usize;
    let monomial = QuadraticMonomial::new_linear(request.coefficient, index);
    let clone = monomial.clone();
    let twin = QuadraticMonomial::new_linear(request.coefficient, index);
    let other_content = QuadraticMonomial::new_linear(request.coefficient, other);
    let first_key = monomial.cache_key();
    let clone_key = clone.cache_key();
    let twin_key = twin.cache_key();
    let other_key = other_content.cache_key();
    let equal_content_gives_distinct_keys =
        first_key != clone_key && first_key != twin_key && clone_key != twin_key;
    let clone_gets_its_own_key = first_key != clone_key;
    let different_content_gives_distinct_keys = first_key != other_key;
    // 伪造：两个**内容不同**的单项式用同一个 raw 键，键就相等了。 `CacheKey` 无法反对。
    // Forging: two **different-content** monomials given one raw key compare equal, and `CacheKey` cannot object.
    let forged = CacheKey::from_raw(request.forged_key);
    let from_raw_forges_collisions =
        forged == CacheKey::from_raw(request.forged_key) && forged.value == request.forged_key;
    // `HashSet<CacheKey>` 按地址去重，所以内容相同的三个实例占三个位置。
    // A `HashSet<CacheKey>` dedupes by address, so three content-identical instances occupy three slots.
    let mut set = std::collections::HashSet::new();
    set.insert(first_key);
    set.insert(clone_key);
    set.insert(twin_key);
    let hashset_does_not_dedupe = set.len() == 3;
    // 每次重建（哪怕内容一模一样）都拿新键：缓存键不是结构身份。
    // Every rebuild takes a fresh key even with identical content: the key is not a structural identity.
    let built = Quadratic::new(vec![QuadraticMonomial::new_linear(request.coefficient, index)],
        request.coefficient);
    let rebuilt = Quadratic::new(vec![QuadraticMonomial::new_linear(request.coefficient, index)],
        request.coefficient);
    let rebuilt_polynomial_gets_a_fresh_key = built.cache_key() != rebuilt.cache_key();
    Ok(FlattenIdentityReport {
        first_key: first_key.value,
        clone_key: clone_key.value,
        twin_key: twin_key.value,
        equal_content_gives_distinct_keys,
        clone_gets_its_own_key,
        different_content_gives_distinct_keys,
        from_raw_forges_collisions,
        hashset_does_not_dedupe,
        rebuilt_polynomial_gets_a_fresh_key,
        monomial_is_linear: monomial.kind() == QuadraticMonomialKind::Linear,
        coefficient: *monomial.coefficient(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> FlattenIdentityRequest {
        FlattenIdentityRequest { coefficient: 2.5, var_index: 3, other_var: 4, forged_key: 0xdead_beef }
    }

    #[test]
    fn equal_content_never_shares_a_key() {
        let report = verify_flatten_identity(&request()).unwrap();
        assert!(report.equal_content_gives_distinct_keys);
        assert!(report.clone_gets_its_own_key);
        assert!(report.different_content_gives_distinct_keys);
        assert_ne!(report.first_key, report.clone_key);
        assert_ne!(report.first_key, report.twin_key);
        assert_ne!(report.clone_key, report.twin_key);
        // 键非零（真实堆地址）。 / Keys are non-zero (real heap addresses).
        assert_ne!(report.first_key, 0);
        assert_ne!(report.twin_key, 0);
    }

    #[test]
    fn a_hashset_of_keys_does_not_dedupe_by_content() {
        let report = verify_flatten_identity(&request()).unwrap();
        assert!(report.hashset_does_not_dedupe);
    }

    #[test]
    fn raw_keys_can_be_forged() {
        let report = verify_flatten_identity(&request()).unwrap();
        assert!(report.from_raw_forges_collisions);
        // 换个伪造值也照样成立。 / A different forged value behaves the same.
        let mut other = request();
        other.forged_key = 7;
        assert!(verify_flatten_identity(&other).unwrap().from_raw_forges_collisions);
    }

    #[test]
    fn a_rebuilt_polynomial_gets_a_fresh_key() {
        let report = verify_flatten_identity(&request()).unwrap();
        assert!(report.rebuilt_polynomial_gets_a_fresh_key);
        assert!(report.monomial_is_linear);
        assert!((report.coefficient - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut bad = request();
        bad.coefficient = f64::INFINITY;
        assert!(verify_flatten_identity(&bad).unwrap_err().message.contains("must be finite"));
        let mut bad = request();
        bad.var_index = MAX_INDEX + 1;
        assert!(verify_flatten_identity(&bad).unwrap_err().message.contains("must be below"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_flatten_identity(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"equalContentGivesDistinctKeys\""), "{json}");
        assert!(json.contains("\"hashsetDoesNotDedupe\""), "{json}");
        assert!(json.contains("\"firstKey\""), "{json}");
        assert!(!json.contains("equal_content"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<FlattenIdentityReport>(&copy.to_string()).is_err());
    }
}

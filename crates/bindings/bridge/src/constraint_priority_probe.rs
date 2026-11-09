//! 续挖第 86 单：**约束优先级的双层身份**——枚举身份与存储值是两层，`value()` **不是单射**：
//! `Custom(0/10/50/100)` 与四个有名变体共享同一个存储值，而 `From<u32>` 只认那一个"规范"代表
//! （`model/basic/constraint_priority.rs` 的 `ConstraintPriority` 与其统计 trait）
//! Continuation, delivery 86: the **two-layer identity** of a constraint priority — the enum identity and the storage
//! value are two layers, and `value()` is **not injective**: `Custom(0/10/50/100)` share a storage value with the four
//! named variants while `From<u32>` only accepts the one "canonical" representative
//! (`ConstraintPriority` and its stats trait in `model/basic/constraint_priority.rs`).
//!
//! 模型层此前只绑过行与元数据（第 62 单量的是八个平行向量），这一单量**优先级这一列自己的语义**。八条实测：
//!
//! 1. **有名阶梯是 `0 / 10 / 50 / 100`**（`the_named_ladder_is_0_10_50_100()`）：`None` / `NiceToHave` /
//!    `Suggested` / `Mandatory`，间隔递增（10、40、50）。
//! 2. **存储往返只有一个方向精确**（`the_storage_round_trip_is_exact_in_one_direction()`）：
//!    `From(v).value() == v` 对**每个** `u32` 成立，但反方向不成立（见第 3 条）。
//! 3. **恰好四个存储值有两个表示**（`four_storage_values_have_two_representations()`）：`Custom(0/10/50/100)`
//!    与对应有名变体 `value()` 相同、`PartialEq` 不同，于是 `value()` **不是单射**。本单最值钱的一条。
//! 4. **`is_non_null()` 就是"存储值非零"**（`is_non_null_is_exactly_a_nonzero_value()`）：`Custom(0)` 也算空。
//! 5. **自定义优先级能落在阶梯任何位置**（`custom_priorities_sit_anywhere_on_the_ladder()`）：
//!    `Custom(7)` 在 `None(0)` 与 `NiceToHave(10)` 之间，`Custom(101)` 在 `Mandatory` 之上。
//! 6. **统计 trait 走 `LinearTriadModel` 时纯委托**（`the_linear_wrapper_delegates_to_the_basic_count()`）。
//! 7. **统计读的是存储值而不是变体**（`the_count_reads_storage_values_not_variants()`）：
//!    `Mandatory` 计数、`None` 与 `Custom(0)` 都不计，且同值的不同变体计数相同。
//! 8. **包装从不改写存储值**（`the_wrapper_never_changes_the_storage_value()`）：
//!    `u32::from(p) == p.value()`，含 `Custom(u32::MAX)`。
//!
//! The model layer had only been bound for rows and metadata (delivery 62 measured the eight parallel vectors); this
//! delivery measures **the semantics of the priority column itself**. **The named ladder is `0 / 10 / 50 / 100`** with
//! increasing gaps (10, 40, 50). **The storage round trip is exact in one direction only** — `From(v).value() == v` holds
//! for **every** `u32` while the reverse does not. **Exactly four storage values have two representations** —
//! `Custom(0/10/50/100)` share `value()` with the corresponding named variant yet differ under `PartialEq`, so `value()`
//! is **not injective**; this is the most valuable finding here. **`is_non_null()` is exactly a non-zero storage value**,
//! so `Custom(0)` counts as null. **Custom priorities can sit anywhere on the ladder.** **The `LinearTriadModel` stat is a
//! pure delegation.** **The count reads storage values rather than variants.** And **the wrapper never rewrites the storage
//! value**, including `Custom(u32::MAX)`.

use ospf_rust_core::model::basic::constraint_priority::{
    ConstraintPriority, ConstraintPriorityStats,
};
use ospf_rust_core::model::intermediate::{BasicLinearTriadModel, LinearTriadModel, SparseVector};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 约束优先级探针请求。 / The constraint-priority probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PriorityRequest {
    /// 依次挂到约束上的存储优先级值。 / The storage priority values put on the constraints in order.
    #[serde(default)]
    pub priorities: Vec<u32>,
}

/// 一条优先级的读面。 / One priority's readable face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PriorityView {
    /// 请求里的存储值。 / The storage value from the request.
    pub storage: u32,
    /// `From<u32>` 得到的变体名：`none` / `nice-to-have` / `suggested` / `mandatory` / `custom`。
    /// The variant name reached through `From<u32>`.
    pub variant: String,
    /// 该变体是否非空。 / Whether this variant is non-null.
    pub non_null: bool,
    /// 规范代表再写回存储值（应等于 `storage`）。 / The canonical representative written back to storage.
    pub canonical_storage: u32,
    /// 变体的 `Debug` 文本（看得出 `Custom` 带的载荷）。 / The variant's `Debug` text, showing a `Custom` payload.
    pub variant_debug: String,
}

/// 约束优先级探针结论。 / The constraint-priority probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PriorityReport {
    /// 每条优先级的读面。 / Each priority's readable face.
    pub entries: Vec<PriorityView>,
    /// 全部条目里"非空"的条数（按 `is_non_null`）。 / How many entries are non-null by `is_non_null`.
    pub non_null_count: u32,
    /// `BasicLinearTriadModel` 上的统计值。 / The stat on `BasicLinearTriadModel`.
    pub basic_count: u32,
    /// `LinearTriadModel` 上的统计值。 / The stat on `LinearTriadModel`.
    pub linear_count: u32,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub the_named_ladder_is_0_10_50_100: bool,
    pub the_storage_round_trip_is_exact_in_one_direction: bool,
    pub four_storage_values_have_two_representations: bool,
    pub is_non_null_is_exactly_a_nonzero_value: bool,
    pub custom_priorities_sit_anywhere_on_the_ladder: bool,
    pub the_linear_wrapper_delegates_to_the_basic_count: bool,
    pub the_count_reads_storage_values_not_variants: bool,
    pub the_wrapper_never_changes_the_storage_value: bool,
}

/// 变体名。 / The variant name.
fn variant_name(priority: ConstraintPriority) -> &'static str {
    match priority {
        ConstraintPriority::None => "none",
        ConstraintPriority::NiceToHave => "nice-to-have",
        ConstraintPriority::Suggested => "suggested",
        ConstraintPriority::Mandatory => "mandatory",
        ConstraintPriority::Custom(_) => "custom",
    }
}

/// 造一个带优先级的线性模型。 / Build a linear model carrying priorities.
fn model_of(priorities: &[u32]) -> (BasicLinearTriadModel, LinearTriadModel) {
    let mut basic = BasicLinearTriadModel::new("priority-probe");
    for (index, priority) in priorities.iter().enumerate() {
        basic.add_constraint_with_metadata(SparseVector::new(), 0.0, format!("c{index}"),
            None, false, *priority, None, None);
    }
    let linear = LinearTriadModel::from_basic(basic.clone());
    (basic, linear)
}

/// 走一遍优先级探针。非法输入是**错误**。 / Walk the priority probe.
pub fn verify_constraint_priority(request: &PriorityRequest) -> Result<PriorityReport> {
    if request.priorities.len() > 32 {
        return Err(BridgeError::new("InvalidInput",
            "constraint priority failed validation: at most 32 priorities are accepted"));
    }
    let findings = canonical_findings();
    let entries = request.priorities.iter().map(|storage| {
        let variant = ConstraintPriority::from(*storage);
        PriorityView {
            storage: *storage,
            variant: variant_name(variant).to_owned(),
            non_null: variant.is_non_null(),
            canonical_storage: variant.value(),
            variant_debug: format!("{variant:?}"),
        }
    }).collect::<Vec<_>>();
    let non_null_count = u32::try_from(entries.iter().filter(|entry| entry.non_null).count())
        .unwrap_or(u32::MAX);
    let (basic, linear) = model_of(&request.priorities);
    Ok(PriorityReport {
        entries,
        non_null_count,
        basic_count: u32::try_from(basic.non_null_constraint_priority_count())
            .unwrap_or(u32::MAX),
        linear_count: u32::try_from(linear.non_null_constraint_priority_count())
            .unwrap_or(u32::MAX),
        the_named_ladder_is_0_10_50_100: findings.the_named_ladder_is_0_10_50_100,
        the_storage_round_trip_is_exact_in_one_direction:
            findings.the_storage_round_trip_is_exact_in_one_direction,
        four_storage_values_have_two_representations:
            findings.four_storage_values_have_two_representations,
        is_non_null_is_exactly_a_nonzero_value: findings.is_non_null_is_exactly_a_nonzero_value,
        custom_priorities_sit_anywhere_on_the_ladder:
            findings.custom_priorities_sit_anywhere_on_the_ladder,
        the_linear_wrapper_delegates_to_the_basic_count:
            findings.the_linear_wrapper_delegates_to_the_basic_count,
        the_count_reads_storage_values_not_variants:
            findings.the_count_reads_storage_values_not_variants,
        the_wrapper_never_changes_the_storage_value:
            findings.the_wrapper_never_changes_the_storage_value,
    })
}

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    the_named_ladder_is_0_10_50_100: bool,
    the_storage_round_trip_is_exact_in_one_direction: bool,
    four_storage_values_have_two_representations: bool,
    is_non_null_is_exactly_a_nonzero_value: bool,
    custom_priorities_sit_anywhere_on_the_ladder: bool,
    the_linear_wrapper_delegates_to_the_basic_count: bool,
    the_count_reads_storage_values_not_variants: bool,
    the_wrapper_never_changes_the_storage_value: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 有名阶梯是 0 / 10 / 50 / 100。 / The named ladder is 0 / 10 / 50 / 100.
    let the_named_ladder_is_0_10_50_100 = {
        ConstraintPriority::None.value() == 0
            && ConstraintPriority::NiceToHave.value() == 10
            && ConstraintPriority::Suggested.value() == 50
            && ConstraintPriority::Mandatory.value() == 100
            // 间隔递增：10、40、50。 / The gaps increase: 10, 40, 50.
            && ConstraintPriority::NiceToHave.value() - ConstraintPriority::None.value() == 10
            && ConstraintPriority::Suggested.value() - ConstraintPriority::NiceToHave.value() == 40
            && ConstraintPriority::Mandatory.value() - ConstraintPriority::Suggested.value() == 50
    };
    // 2) 存储往返只有一个方向精确。 / The storage round trip is exact in one direction only.
    let the_storage_round_trip_is_exact_in_one_direction = {
        // `From(v).value() == v` 对每个采样值成立。 / `From(v).value() == v` for every sampled value.
        [0_u32, 1, 7, 10, 49, 50, 100, 101, u32::MAX].iter().all(|storage| {
            ConstraintPriority::from(*storage).value() == *storage
        })
    };
    // 3) 恰好四个存储值有两个表示。 / Exactly four storage values have two representations.
    let four_storage_values_have_two_representations = {
        let pairs = [(0_u32, ConstraintPriority::None),
            (10, ConstraintPriority::NiceToHave), (50, ConstraintPriority::Suggested),
            (100, ConstraintPriority::Mandatory)];
        // 每个有名变体都有一个 `Custom` 兄弟共享存储值，而 `PartialEq` 判不同。 
        // Every named variant has a `Custom` sibling sharing its storage value while `PartialEq` says they differ.
        let collapsing = pairs.iter().filter(|(storage, named)| {
            ConstraintPriority::Custom(*storage).value() == named.value()
                && ConstraintPriority::Custom(*storage) != *named
        }).count();
        // 其余值只有一个表示：`From(v)` 就是 `Custom(v)`。 
        // Every other value has one representation: `From(v)` is `Custom(v)`.
        let unique = [1_u32, 7, 49, 101, u32::MAX].iter().all(|storage| {
            *storage != 0 && ConstraintPriority::from(*storage) == ConstraintPriority::custom(*storage)
        });
        collapsing == 4 && unique
            // `value()` 因此不是单射：两个不同变体、一个存储值。 
            // `value()` is therefore not injective: two distinct variants share one storage value.
            && ConstraintPriority::Custom(10) != ConstraintPriority::NiceToHave
            && ConstraintPriority::Custom(10).value() == ConstraintPriority::NiceToHave.value()
    };
    // 4) `is_non_null()` 就是"存储值非零"。 / `is_non_null()` is exactly a non-zero storage value.
    let is_non_null_is_exactly_a_nonzero_value = {
        // `Custom(0)` 也算空。 / `Custom(0)` counts as null too.
        !ConstraintPriority::None.is_non_null()
            && !ConstraintPriority::Custom(0).is_non_null()
            && ConstraintPriority::NiceToHave.is_non_null()
            && ConstraintPriority::Custom(1).is_non_null()
            // 与 `value() != 0` 完全一致。 / Exactly equivalent to `value() != 0`.
            && [ConstraintPriority::None, ConstraintPriority::Custom(0),
                ConstraintPriority::NiceToHave, ConstraintPriority::Mandatory,
                ConstraintPriority::Custom(u32::MAX)]
                .iter().all(|priority| priority.is_non_null() == (priority.value() != 0))
    };
    // 5) 自定义优先级能落在阶梯任何位置。 / Custom priorities can sit anywhere on the ladder.
    let custom_priorities_sit_anywhere_on_the_ladder = {
        // `Custom(7)` 夹在 `None(0)` 与 `NiceToHave(10)` 之间；`Custom(101)` 在 `Mandatory(100)` 之上。 
        // `Custom(7)` sits between `None(0)` and `NiceToHave(10)` while `Custom(101)` sits above `Mandatory(100)`.
        ConstraintPriority::Custom(7).value()
            > ConstraintPriority::None.value()
            && ConstraintPriority::Custom(7).value()
                < ConstraintPriority::NiceToHave.value()
            && ConstraintPriority::Custom(101).value()
                > ConstraintPriority::Mandatory.value()
    };
    // 6) `LinearTriadModel` 上的统计纯委托。 / The stat on `LinearTriadModel` is a pure delegation.
    let the_linear_wrapper_delegates_to_the_basic_count = {
        let priorities = [0_u32, 50, 7, 0];
        let (basic, linear) = model_of(&priorities);
        basic.non_null_constraint_priority_count()
            == linear.non_null_constraint_priority_count()
            // 也只等于"非零个数"。 / It also equals the count of non-zero values.
            && basic.non_null_constraint_priority_count() == 2
    };
    // 7) 统计读的是存储值而不是变体。 / The count reads storage values rather than variants.
    let the_count_reads_storage_values_not_variants = {
        // `Mandatory` 与时数 100 计一样多，`Custom(0)` 与 `None` 都不计。 
        // `Mandatory` counts the same as the number 100, while `Custom(0)` and `None` count for nothing.
        let by_variant = [u32::from(ConstraintPriority::Mandatory),
            u32::from(ConstraintPriority::None), 0];
        let by_value = [100_u32, 0, 0];
        let (basic_variant, _) = model_of(&by_variant);
        let (basic_value, _) = model_of(&by_value);
        basic_variant.non_null_constraint_priority_count() == 1
            && basic_value.non_null_constraint_priority_count() == 1
            // 空串对照：全零则 0。 / Control: all zeros count to zero.
            && model_of(&[0, 0, 0]).0.non_null_constraint_priority_count() == 0
    };
    // 8) 包装从不改写存储值。 / The wrapper never changes the storage value.
    let the_wrapper_never_changes_the_storage_value = {
        let variants = [ConstraintPriority::None, ConstraintPriority::NiceToHave,
            ConstraintPriority::Suggested, ConstraintPriority::Mandatory,
            ConstraintPriority::Custom(7), ConstraintPriority::Custom(u32::MAX)];
        // `u32::from(p) == p.value()`。 / `u32::from(p) == p.value()`.
        variants.iter().all(|priority| u32::from(*priority) == (*priority).value())
            && u32::from(ConstraintPriority::Custom(u32::MAX)) == u32::MAX
    };
    Findings {
        the_named_ladder_is_0_10_50_100,
        the_storage_round_trip_is_exact_in_one_direction,
        four_storage_values_have_two_representations,
        is_non_null_is_exactly_a_nonzero_value,
        custom_priorities_sit_anywhere_on_the_ladder,
        the_linear_wrapper_delegates_to_the_basic_count,
        the_count_reads_storage_values_not_variants,
        the_wrapper_never_changes_the_storage_value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> PriorityRequest {
        PriorityRequest { priorities: Vec::new() }
    }

    #[test]
    fn an_empty_request_keeps_the_eight_rules_readable() {
        let report = verify_constraint_priority(&request()).unwrap();
        assert!(report.entries.is_empty());
        assert_eq!(report.non_null_count, 0);
        assert_eq!(report.basic_count, 0);
        assert_eq!(report.linear_count, 0);
        assert!(report.the_named_ladder_is_0_10_50_100);
        assert!(report.the_storage_round_trip_is_exact_in_one_direction);
        assert!(report.four_storage_values_have_two_representations);
        assert!(report.is_non_null_is_exactly_a_nonzero_value);
        assert!(report.custom_priorities_sit_anywhere_on_the_ladder);
        assert!(report.the_linear_wrapper_delegates_to_the_basic_count);
        assert!(report.the_count_reads_storage_values_not_variants);
        assert!(report.the_wrapper_never_changes_the_storage_value);
    }

    #[test]
    fn the_four_named_values_round_trip_through_their_variants() {
        let report = verify_constraint_priority(&PriorityRequest {
            priorities: vec![0, 10, 50, 100, 7] }).unwrap();
        assert_eq!(report.entries.iter().map(|entry| entry.variant.clone())
            .collect::<Vec<_>>(),
            vec!["none", "nice-to-have", "suggested", "mandatory", "custom"]);
        assert_eq!(report.entries.iter().map(|entry| entry.canonical_storage)
            .collect::<Vec<_>>(), vec![0, 10, 50, 100, 7]);
        assert_eq!(report.entries[4].variant_debug, "Custom(7)");
        assert!(report.entries[0].variant_debug.starts_with("None"));
    }

    #[test]
    fn only_non_zero_values_count() {
        let report = verify_constraint_priority(&PriorityRequest { priorities: vec![0, 50, 7, 0] })
            .unwrap();
        assert_eq!(report.non_null_count, 2);
        assert_eq!(report.basic_count, 2);
        assert_eq!(report.linear_count, 2);
        assert!(!report.entries[0].non_null);
        assert!(report.entries[1].non_null);
        assert!(report.entries[2].non_null);
    }

    #[test]
    fn the_extreme_value_stays_intact() {
        let report = verify_constraint_priority(&PriorityRequest {
            priorities: vec![u32::MAX] }).unwrap();
        assert_eq!(report.entries[0].storage, u32::MAX);
        assert_eq!(report.entries[0].canonical_storage, u32::MAX);
        assert_eq!(report.entries[0].variant, "custom");
        assert!(report.entries[0].non_null);
        assert_eq!(report.basic_count, 1);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_constraint_priority(&PriorityRequest { priorities: (0..33)
            .map(|index| index as u32).collect() }).unwrap_err()
            .message.contains("at most 32 priorities"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        // 用一条真条目，否则 `entries` 是空数组、读面的字段根本不会出现。 
        // Use one real entry; an empty `entries` array would not even carry the readable fields.
        let report = verify_constraint_priority(&PriorityRequest { priorities: vec![10] })
            .unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"canonicalStorage\""), "{json}");
        assert!(json.contains("\"variantDebug\""), "{json}");
        assert!(json.contains("\"theStorageRoundTripIsExactInOneDirection\""), "{json}");
        assert!(json.contains("\"fourStorageValuesHaveTwoRepresentations\""), "{json}");
        assert!(!json.contains("canonical_storage"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<PriorityReport>(&copy.to_string()).is_err());
    }
}

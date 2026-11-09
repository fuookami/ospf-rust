//! 续挖第 47 单：辅助变量的**整数值域校验**——`|界| > 2^53` 一律拒，恰好 `2^53` 放行
//! （`solver/logic_based_benders.rs` 的 `MasterCut::normalize` → `MasterVariableDomain::validate`）
//! Continuation, delivery 47: the **integer-domain validation** of auxiliary variables — any bound with `|bound| > 2^53` is
//! refused while exactly `2^53` passes (`MasterCut::normalize` → `MasterVariableDomain::validate`).
//!
//! 第 33 单量的是有界整数 no-good 族行的**结构与辅助集计数**、第 37 单量**行系数闭式**；这一单量辅助变量声明本身
//! 的域校验。它是 `pub(crate)` 的，所以**只能通过 `MasterCut::normalize()` 观测**。五条实测：
//!
//! 1. **`lower > upper` 被拒**（`inverted_bounds_are_refused()`），消息带上下界原值。
//! 2. **界的绝对值超过 `2^53` 被拒**（`bounds_must_be_exact_in_f64()`）：`exact_f64_integer` 的阈值是
//!    `MAX_EXACT_INTEGER_F64 = 1 << 53`，判据是**严格大于**——所以 `±2^53` 恰好**放行**，
//!    `2^53 + 1` 才被拒。这条是给"`i64` 界转 `f64`"留的天花板：超出后转成 f64 就不再唯一。
//! 3. **`Binary` 域不参与这两项校验**（`binary_is_unchecked()`）：它没有界字段，`contains` 是常量 `0..=1`。
//! 4. **空白辅助 ID 被拒**（`blank_ids_are_refused()`），但**重复 ID 是另一条报错**
//!    （`duplicate_ids_are_refused()`，"declares duplicate auxiliary variable"）——两条检查分属不同位置。
//! 5. **`normalize()` 原样保留辅助声明**（`normalize_keeps_the_declarations()`）：条数、顺序、域都不变。
//!
//! Delivery 33 measured the family rows' **structure and auxiliary count**, delivery 37 their **coefficient closed forms**;
//! this one measures the declaration's own domain validation. That check is `pub(crate)`, so it is **only observable through
//! `MasterCut::normalize()`**. **`lower > upper` is refused** (`inverted_bounds_are_refused()`) with both bounds quoted in the
//! message. **A bound whose absolute value exceeds `2^53` is refused** (`bounds_must_be_exact_in_f64()`):
//! `exact_f64_integer`'s threshold is `MAX_EXACT_INTEGER_F64 = 1 << 53` compared strictly, so `±2^53` **passes** and only
//! `2^53 + 1` is refused — the ceiling that keeps an `i64` bound uniquely representable as `f64`. **The `Binary` domain takes
//! neither check** (`binary_is_unchecked()`), having no bound fields and a constant `0..=1` membership. **A blank auxiliary ID
//! is refused** (`blank_ids_are_refused()`) while **a duplicate ID raises a different error**
//! (`duplicate_ids_are_refused()`, "declares duplicate auxiliary variable"), so the two checks live in different places. And
//! **`normalize()` keeps the declarations verbatim** (`normalize_keeps_the_declarations()`): same count, order and domains.

use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_framework::CutSense;
use ospf_rust_framework::solver::logic_based_benders::{
    MasterCut, MasterCutProofStatus, MasterCutValidity, MasterVariableBinding, MasterVariableDomain,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个辅助变量声明的字面量。 / One auxiliary declaration literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuxiliarySpec {
    /// 辅助变量 ID。 / The auxiliary variable ID.
    pub id: String,
    /// 域：`binary` 或 `integer`。 / The domain: `binary` or `integer`.
    #[serde(default)]
    pub kind: Option<String>,
    /// `integer` 的下界。 / The `integer` lower bound.
    #[serde(default)]
    pub lower: Option<i64>,
    /// `integer` 的上界。 / The `integer` upper bound.
    #[serde(default)]
    pub upper: Option<i64>,
}

/// 一次声明的校验结果。 / One declaration's validation result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuxiliaryOutcome {
    /// 辅助变量 ID。 / The auxiliary variable ID.
    pub id: String,
    /// 单独一条声明能否通过 `normalize()`。 / Whether the declaration alone survives `normalize()`.
    pub accepted: bool,
    /// 失败消息。 / The failure message.
    #[serde(default)]
    pub error: Option<String>,
}

/// 辅助域校验请求。 / The auxiliary-domain request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuxiliaryDomainRequest {
    /// 要逐条试的声明。 / The declarations to try one by one.
    #[serde(default)]
    pub auxiliaries: Vec<AuxiliarySpec>,
    /// 是否把全部声明一次性放进同一个 cut（量重复 ID）。 / Whether to put every declaration into one cut (to measure
    /// duplicate IDs).
    #[serde(default)]
    pub together: bool,
}

/// 辅助域校验结论。 / The auxiliary-domain verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuxiliaryDomainReport {
    /// 逐条结果。 / One outcome per declaration.
    pub outcomes: Vec<AuxiliaryOutcome>,
    /// 一次性放全部时的结果（`together` 为假时是 `null`）。 / The result of one cut holding them all (`null` when
    /// `together` is false).
    #[serde(default)]
    pub combined_accepted: Option<bool>,
    /// 一次性放全部时的失败消息。 / The combined failure message.
    #[serde(default)]
    pub combined_error: Option<String>,
    /// `normalize()` 之后保留的辅助声明条数。 / The declaration count kept after `normalize()`.
    pub kept_count: u64,
    /// `normalize()` 之后保留的辅助 ID（按原顺序）。 / The IDs kept after `normalize()` (in order).
    pub kept_ids: Vec<String>,
    /// 恰好 `2^53` 被放行（**是**）。 / Whether exactly `2^53` passes (**yes**).
    pub two_to_the_53_passes: bool,
    /// `2^53 + 1` 被拒（**是**）。 / Whether `2^53 + 1` is refused (**yes**).
    pub beyond_the_exact_range_is_refused: bool,
    /// `lower > upper` 被拒（**是**）。 / Whether inverted bounds are refused (**yes**).
    pub inverted_bounds_are_refused: bool,
    /// `binary` 域不参与校验（**是**）。 / Whether the `binary` domain is unchecked (**yes**).
    pub binary_is_unchecked: bool,
    /// 空白 ID 被拒（**是**）。 / Whether blank IDs are refused (**yes**).
    pub blank_ids_are_refused: bool,
    /// 重复 ID 是另一条报错（**是**）。 / Whether a duplicate ID raises a different error (**yes**).
    pub duplicate_ids_are_refused: bool,
    /// `normalize()` 原样保留声明（**是**）。 / Whether `normalize()` keeps the declarations verbatim (**yes**).
    pub normalize_keeps_the_declarations: bool,
}

/// 声明条数上限（信任边界）。 / The declaration cap (a trust boundary).
const MAX_AUXILIARIES: usize = 64;

/// 造一条声明。 / Build one binding.
fn binding(spec: &AuxiliarySpec) -> Result<MasterVariableBinding> {
    let id = StableVariableId(spec.id.clone());
    match spec.kind.as_deref().unwrap_or("binary") {
        "binary" => Ok(MasterVariableBinding { id, domain: MasterVariableDomain::Binary }),
        "integer" => {
            let lower = spec.lower.ok_or_else(|| BridgeError::new("InvalidInput",
                "auxiliary domain failed validation: an integer domain needs a lower bound"))?;
            let upper = spec.upper.ok_or_else(|| BridgeError::new("InvalidInput",
                "auxiliary domain failed validation: an integer domain needs an upper bound"))?;
            Ok(MasterVariableBinding { id, domain: MasterVariableDomain::Integer { lower, upper } })
        }
        other => Err(BridgeError::new("InvalidInput", &format!(
            "auxiliary domain failed validation: unknown domain `{other}`"))),
    }
}

/// 造一个只带这些辅助声明的 cut。 / Build a cut carrying only these declarations.
fn cut_with(bindings: Vec<MasterVariableBinding>) -> MasterCut {
    let mut cut = MasterCut::new("aux-probe", vec![(StableVariableId("x".to_owned()), 1.0)],
        CutSense::LessOrEqual, 1.0, MasterCutValidity::Global, MasterCutProofStatus::None, None,
        "probe", "probe");
    cut.auxiliary_variables = bindings;
    cut
}

/// 走一遍辅助域校验。非法输入是**错误**。 / Walk the auxiliary-domain check.
pub fn verify_auxiliary_domain(request: &AuxiliaryDomainRequest)
    -> Result<AuxiliaryDomainReport>
{
    if request.auxiliaries.len() > MAX_AUXILIARIES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "auxiliary domain failed validation: at most {MAX_AUXILIARIES} declarations are accepted")));
    }
    let bindings = request.auxiliaries.iter().map(binding).collect::<Result<Vec<_>>>()?;
    let outcomes: Vec<AuxiliaryOutcome> = request.auxiliaries.iter().zip(bindings.iter())
        .map(|(spec, declaration)| {
            let outcome = cut_with(vec![declaration.clone()]).normalize();
            match outcome {
                Ok(_) => AuxiliaryOutcome { id: spec.id.clone(), accepted: true, error: None },
                Err(failure) => AuxiliaryOutcome { id: spec.id.clone(), accepted: false,
                    error: Some(failure.to_string()) },
            }
        }).collect();
    let (combined_accepted, combined_error) = if request.together {
        match cut_with(bindings.clone()).normalize() {
            Ok(_) => (Some(true), None),
            Err(failure) => (Some(false), Some(failure.to_string())),
        }
    } else {
        (None, None)
    };
    let normalized = cut_with(bindings);
    let kept = normalized.normalize();
    let (kept_ids, kept_count) = match &kept {
        Ok(value) => (value.auxiliary_variables.iter()
            .map(|declaration| declaration.id.0.clone()).collect::<Vec<_>>(),
            value.auxiliary_variables.len() as u64),
        Err(_) => (Vec::new(), 0),
    };
    let findings = canonical_findings();
    Ok(AuxiliaryDomainReport { outcomes, combined_accepted, combined_error, kept_count, kept_ids,
        two_to_the_53_passes: findings.two_to_the_53_passes,
        beyond_the_exact_range_is_refused: findings.beyond_the_exact_range_is_refused,
        inverted_bounds_are_refused: findings.inverted_bounds_are_refused,
        binary_is_unchecked: findings.binary_is_unchecked,
        blank_ids_are_refused: findings.blank_ids_are_refused,
        duplicate_ids_are_refused: findings.duplicate_ids_are_refused,
        normalize_keeps_the_declarations: findings.normalize_keeps_the_declarations })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    two_to_the_53_passes: bool,
    beyond_the_exact_range_is_refused: bool,
    inverted_bounds_are_refused: bool,
    binary_is_unchecked: bool,
    blank_ids_are_refused: bool,
    duplicate_ids_are_refused: bool,
    normalize_keeps_the_declarations: bool,
}

/// `2^53`——`exact_f64_integer` 的阈值。 / `2^53`, the `exact_f64_integer` threshold.
const EXACT_LIMIT: i64 = 1_i64 << 53;

/// 造一条整数声明。 / Build one integer declaration.
fn integer(id: &str, lower: i64, upper: i64) -> MasterVariableBinding {
    MasterVariableBinding { id: StableVariableId(id.to_owned()),
        domain: MasterVariableDomain::Integer { lower, upper } }
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    // 1) 恰好 `2^53` 放行，`2^53 + 1` 被拒；负侧对称。 
    // 1) Exactly `2^53` passes, `2^53 + 1` is refused; the negative side is symmetric.
    let two_to_the_53_passes = cut_with(vec![integer("a", 0, EXACT_LIMIT)]).normalize().is_ok()
        && cut_with(vec![integer("a", -EXACT_LIMIT, 0)]).normalize().is_ok();
    let beyond_the_exact_range_is_refused =
        cut_with(vec![integer("a", 0, EXACT_LIMIT + 1)]).normalize().is_err()
            && cut_with(vec![integer("a", -EXACT_LIMIT - 1, 0)]).normalize().is_err();
    // 2) 界颠倒被拒。 / Inverted bounds are refused.
    let inverted_bounds_are_refused = cut_with(vec![integer("a", 5, 1)]).normalize().is_err()
        && cut_with(vec![integer("a", 1, 1)]).normalize().is_ok();
    // 3) `Binary` 域不参与这两项校验。 / The `Binary` domain takes neither check.
    let binary_is_unchecked = {
        let binary = MasterVariableBinding { id: StableVariableId("b".to_owned()),
            domain: MasterVariableDomain::Binary };
        cut_with(vec![binary]).normalize().is_ok()
    };
    // 4) 空白 ID 被拒。 / A blank ID is refused.
    let blank_ids_are_refused = {
        let blank = MasterVariableBinding { id: StableVariableId("   ".to_owned()),
            domain: MasterVariableDomain::Binary };
        match cut_with(vec![blank]).normalize() {
            Err(failure) => failure.to_string().contains("ID must not be empty"),
            Ok(_) => false,
        }
    };
    // 5) 重复 ID 是另一条报错。 / A duplicate ID raises a different error.
    let duplicate_ids_are_refused = {
        let binary = |id: &str| MasterVariableBinding { id: StableVariableId(id.to_owned()),
            domain: MasterVariableDomain::Binary };
        match cut_with(vec![binary("dup"), binary("dup")]).normalize() {
            Err(failure) => failure.to_string().contains("duplicate auxiliary variable"),
            Ok(_) => false,
        }
    };
    // 6) `normalize()` 原样保留：条数、顺序、域都不变。 
    // 6) `normalize()` keeps them verbatim: count, order and domains unchanged.
    let normalize_keeps_the_declarations = {
        let declarations = vec![integer("z", 0, 3),
            MasterVariableBinding { id: StableVariableId("y".to_owned()),
                domain: MasterVariableDomain::Binary }];
        match cut_with(declarations.clone()).normalize() {
            Ok(value) => value.auxiliary_variables == declarations,
            Err(_) => false,
        }
    };
    Findings { two_to_the_53_passes, beyond_the_exact_range_is_refused,
        inverted_bounds_are_refused, binary_is_unchecked, blank_ids_are_refused,
        duplicate_ids_are_refused, normalize_keeps_the_declarations }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AuxiliaryDomainRequest {
        AuxiliaryDomainRequest { auxiliaries: vec![
            AuxiliarySpec { id: "ok".to_owned(), kind: Some("integer".to_owned()),
                lower: Some(0), upper: Some(3) },
            AuxiliarySpec { id: "bin".to_owned(), kind: Some("binary".to_owned()), lower: None,
                upper: None },
        ], together: false }
    }

    #[test]
    fn a_valid_domain_passes_and_is_kept_verbatim() {
        let report = verify_auxiliary_domain(&request()).unwrap();
        assert!(report.outcomes.iter().all(|outcome| outcome.accepted));
        assert_eq!(report.kept_count, 2);
        // 顺序原样保留。 / The order is kept verbatim.
        assert_eq!(report.kept_ids, vec!["ok".to_owned(), "bin".to_owned()]);
        assert!(report.normalize_keeps_the_declarations);
        assert!(report.binary_is_unchecked);
        assert!(report.two_to_the_53_passes);
        assert!(report.beyond_the_exact_range_is_refused);
        assert!(report.inverted_bounds_are_refused);
        assert!(report.blank_ids_are_refused);
        assert!(report.duplicate_ids_are_refused);
    }

    #[test]
    fn the_exact_range_ceiling_is_two_to_the_53() {
        // 恰好 2^53 放行。 / Exactly 2^53 passes.
        let at_limit = AuxiliaryDomainRequest { auxiliaries: vec![AuxiliarySpec {
            id: "a".to_owned(), kind: Some("integer".to_owned()), lower: Some(0),
            upper: Some(EXACT_LIMIT) }], together: false };
        assert!(verify_auxiliary_domain(&at_limit).unwrap().outcomes[0].accepted);
        // 2^53 + 1 被拒。 / 2^53 + 1 is refused.
        let beyond = AuxiliaryDomainRequest { auxiliaries: vec![AuxiliarySpec {
            id: "a".to_owned(), kind: Some("integer".to_owned()), lower: Some(0),
            upper: Some(EXACT_LIMIT + 1) }], together: false };
        let report = verify_auxiliary_domain(&beyond).unwrap();
        assert!(!report.outcomes[0].accepted);
        assert!(report.outcomes[0].error.as_deref().unwrap_or_default()
            .contains("exceeds the exact f64 integer range"));
    }

    #[test]
    fn inverted_bounds_are_refused_with_both_values() {
        let document = AuxiliaryDomainRequest { auxiliaries: vec![AuxiliarySpec {
            id: "a".to_owned(), kind: Some("integer".to_owned()), lower: Some(5),
            upper: Some(1) }], together: false };
        let report = verify_auxiliary_domain(&document).unwrap();
        assert!(!report.outcomes[0].accepted);
        let message = report.outcomes[0].error.as_deref().unwrap_or_default();
        assert!(message.contains("lower 5 above upper 1"), "{message}");
    }

    #[test]
    fn a_blank_id_and_a_duplicate_id_fail_differently() {
        let blank = AuxiliaryDomainRequest { auxiliaries: vec![AuxiliarySpec {
            id: "  ".to_owned(), kind: Some("binary".to_owned()), lower: None, upper: None }],
            together: false };
        let report = verify_auxiliary_domain(&blank).unwrap();
        assert!(report.outcomes[0].error.as_deref().unwrap_or_default()
            .contains("ID must not be empty"));
        // 重复 ID 只有"放在一起"才出现。 / A duplicate ID only shows up when they share a cut.
        let duplicated = AuxiliaryDomainRequest { auxiliaries: vec![
            AuxiliarySpec { id: "dup".to_owned(), kind: Some("binary".to_owned()),
                lower: None, upper: None },
            AuxiliarySpec { id: "dup".to_owned(), kind: Some("binary".to_owned()),
                lower: None, upper: None }], together: true };
        let report = verify_auxiliary_domain(&duplicated).unwrap();
        // 逐条都过（各自只有一条）。 / Each passes alone (one declaration each).
        assert!(report.outcomes.iter().all(|outcome| outcome.accepted));
        assert_eq!(report.combined_accepted, Some(false));
        assert!(report.combined_error.as_deref().unwrap_or_default()
            .contains("duplicate auxiliary variable"));
    }

    #[test]
    fn bad_input_is_refused() {
        let reversed = AuxiliaryDomainRequest { auxiliaries: vec![AuxiliarySpec {
            id: "a".to_owned(), kind: Some("integer".to_owned()), lower: Some(1),
            upper: None }], together: false };
        assert!(verify_auxiliary_domain(&reversed).unwrap_err().message
            .contains("needs an upper bound"));
        let unknown = AuxiliaryDomainRequest { auxiliaries: vec![AuxiliarySpec {
            id: "a".to_owned(), kind: Some("real".to_owned()), lower: None, upper: None }],
            together: false };
        assert!(verify_auxiliary_domain(&unknown).unwrap_err().message
            .contains("unknown domain"));
        let too_many = AuxiliaryDomainRequest { auxiliaries: (0..65).map(|index| AuxiliarySpec {
            id: format!("a{index}"), kind: Some("binary".to_owned()), lower: None, upper: None })
            .collect(), together: false };
        assert!(verify_auxiliary_domain(&too_many).unwrap_err().message
            .contains("at most 64 declarations"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_auxiliary_domain(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"keptCount\""), "{json}");
        assert!(json.contains("\"twoToThe53Passes\""), "{json}");
        assert!(json.contains("\"beyondTheExactRangeIsRefused\""), "{json}");
        assert!(!json.contains("kept_count"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<AuxiliaryDomainReport>(&copy.to_string()).is_err());
    }
}

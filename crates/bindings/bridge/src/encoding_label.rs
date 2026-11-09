//! 续挖第 43 单：`MasterCutEncoding` 是**元数据**——`expressionKey()` 与 `normalize()` 都看不见它，
//! 只有 `PartialEq` 看得见（`solver/logic_based_benders.rs`）
//! Continuation, delivery 43: `MasterCutEncoding` is **metadata** — neither `expressionKey()` nor `normalize()` sees it,
//! only `PartialEq` does (`solver/logic_based_benders.rs`).
//!
//! 第 33 单量的是有界整数 no-good 的**族结构**、第 37 单量的是**行系数闭式**；这一单量"这条 cut 自称是哪种编码"
//! 这件事本身有什么后果。四条实测（全部可达、可判定）：
//!
//! 1. **`MasterCut::new` 恒给 `Linear`**（`new_always_claims_linear()`）：要别的编码必须**手动写字段**，
//!    构造函数没有对应参数。
//! 2. **`expression_key()` 完全不读 encoding**（`key_ignores_the_encoding()`）：两个表达式完全相同、
//!    只有编码不同的 cut 得到**同一个 key**。所以"key 相同"**不**意味着"编码相同"——
//!    按 key 去重会把线性行与二元 no-good 行当成同一个表达式。
//! 3. **`normalize()` 也不校验 encoding**（`normalize_ignores_the_encoding()`）：声明成
//!    `BoundedIntegerNoGood` 而 `assignment` 为空、`family_id` 只有空格，`normalize()` 照样返回 `Ok`；
//!    而且 `family_id` **原样保留**（`normalize()` 只 `trim` 证明引用）。编码的**自洽性没有任何地方检查**。
//! 4. **`PartialEq` 读 encoding**（`equality_sees_the_encoding()`）：id/来源/原因等**全部相同**、只有编码不同
//!    → **不相等**。于是"相同表达式"在 `key` 上是同一个、在 `==` 上是两个——两个判据**必然分歧**
//!    （注意 `PartialEq` 是**整结构**比较，所以这条只有在其它字段全同时才测得出来）。
//!
//! **诚实标注**：真正看得见 encoding 的那一处是 `MasterCut::violates_at`（`BoundedIntegerNoGood` 直接报
//! "must be validated as a complete family"）以及批校验里的编码分派，但两者都是**私有**的
//! （`fn violates_at` / `fn normalize_and_validate_cuts`），**本层量不到**。本单不假装量过。
//!
//! Delivery 33 measured the bounded-integer no-good's **family structure** and delivery 37 its **row closed forms**; this
//! one measures what the claim "this cut is encoded such-and-such" itself implies. **`MasterCut::new` always claims
//! `Linear`** (`new_always_claims_linear()`): any other encoding requires writing the field by hand, as the constructor has
//! no parameter for it. **`expression_key()` does not read the encoding at all** (`key_ignores_the_encoding()`): two cuts
//! with identical expressions that differ only in encoding produce the **same key**, so "same key" does **not** mean "same
//! encoding" — deduplicating by key treats a linear row and a binary no-good row as one expression. **`normalize()` does not
//! validate the encoding either** (`normalize_ignores_the_encoding()`): a cut claiming `BoundedIntegerNoGood` with an empty
//! `assignment` and a blank `family_id` still normalizes to `Ok`, so **nothing anywhere checks the encoding's
//! self-consistency**. And **`PartialEq` does read the encoding** (`equality_sees_the_encoding()`): identical expressions with
//! different encodings are **not equal**, so "the same expression" is one thing under `key` and two things under `==` — the
//! two predicates necessarily disagree.
//!
//! **Honest note**: the one place that really does read the encoding is `MasterCut::violates_at` (a `BoundedIntegerNoGood` cut
//! reports "must be validated as a complete family") along with the cut-batch encoding dispatch, but both are **private**
//! (`fn violates_at`, `fn normalize_and_validate_cuts`), so **this layer cannot measure them** — and does not pretend to.

use std::collections::BTreeMap;

use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_framework::CutSense;
use ospf_rust_framework::solver::logic_based_benders::{
    BoundedIntegerNoGoodRow, MasterCut, MasterCutEncoding, MasterCutProofStatus, MasterCutValidity,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个系数字面量。 / One coefficient literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EncodingTerm {
    /// 稳定变量 ID。 / The stable variable ID.
    pub variable: String,
    /// 系数。 / The coefficient.
    pub coefficient: f64,
}

/// 编码标签请求。 / The encoding-label request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EncodingLabelRequest {
    /// 要**声明**的编码：`linear` / `binaryNoGood` / `boundedIntegerNoGood`。
    /// The encoding to **claim**: `linear`, `binaryNoGood` or `boundedIntegerNoGood`.
    #[serde(default)]
    pub encoding: Option<String>,
    /// `binaryNoGood` / `boundedIntegerNoGood` 的赋值（可为空，用来证明不校验自洽性）。
    /// The assignment for `binaryNoGood` / `boundedIntegerNoGood` (may be empty to show nothing is validated).
    #[serde(default)]
    pub assignment: Vec<EncodingAssignmentRow>,
    /// `boundedIntegerNoGood` 的族 ID（可为空白）。 / The family id for `boundedIntegerNoGood` (may be blank).
    #[serde(default)]
    pub family_id: Option<String>,
    /// 系数项。 / The terms.
    #[serde(default)]
    pub terms: Vec<EncodingTerm>,
    /// 右端项。 / The right-hand side.
    #[serde(default)]
    pub rhs: f64,
}

/// 赋值里的一个变量。 / One variable inside the assignment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EncodingAssignmentRow {
    /// 稳定变量 ID。 / The stable variable ID.
    pub variable: String,
    /// 赋的整数值。 / The integer value.
    pub value: i64,
}

/// 编码标签结论。 / The encoding-label verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EncodingLabelReport {
    /// `new` 给的编码标签。 / The label `new` gives.
    pub new_label: String,
    /// 手动声明后的编码标签。 / The label after the manual claim.
    pub claimed_label: String,
    /// `new` 与"手动声明"两个 cut 的 key 是否相同。 / Whether the two cuts' keys agree.
    pub keys_agree: bool,
    /// 两个 cut 是否 `==`。 / Whether the two cuts are `==`.
    pub cuts_are_equal: bool,
    /// 声明的编码是否通过了 `normalize()`。 / Whether the claimed encoding survives `normalize()`.
    pub normalize_accepted: bool,
    /// `normalize()` 之后的编码标签。 / The label after `normalize()`.
    pub normalized_label: String,
    /// `normalize()` 之后的族 ID。 / The family id after `normalize()`.
    pub normalized_family_id: Option<String>,
    /// `new` 恒给线性（**是**）。 / Whether `new` always claims linear (**yes**).
    pub new_always_claims_linear: bool,
    /// key 不读编码（**是**）。 / Whether the key ignores the encoding (**yes**).
    pub key_ignores_the_encoding: bool,
    /// `normalize()` 不校验编码（**是**）。 / Whether `normalize()` does not validate the encoding (**yes**).
    pub normalize_ignores_the_encoding: bool,
    /// `==` 读编码（**是**）。 / Whether `==` sees the encoding (**yes**).
    pub equality_sees_the_encoding: bool,
}

/// 编码标签名。 / The encoding label name.
fn label(encoding: &MasterCutEncoding) -> &'static str {
    match encoding {
        MasterCutEncoding::Linear => "linear",
        MasterCutEncoding::BinaryNoGood { .. } => "binaryNoGood",
        MasterCutEncoding::BoundedIntegerNoGood { .. } => "boundedIntegerNoGood",
    }
}

/// 造一个 cut。**`name` 固定为同一个**，这样"只有编码不同"的两个 cut 才可比——
/// `PartialEq` 是整结构比较，id/来源/原因任何一项不同都会让它为假。
/// Build a cut. **The `name` is pinned** so that two cuts differing only in encoding are comparable: `PartialEq` compares
/// the whole struct, so any difference in id, origin or reason alone would make it false.
fn cut(request: &EncodingLabelRequest, name: &str) -> MasterCut {
    MasterCut::new(name, request.terms.iter().map(|term| {
        (StableVariableId(term.variable.clone()), term.coefficient)
    }).collect::<Vec<_>>(), CutSense::LessOrEqual, request.rhs, MasterCutValidity::Global,
        MasterCutProofStatus::None, None, "probe", "probe")
}

/// 按请求声明编码。 / Claim the requested encoding.
fn claim(cut: MasterCut, request: &EncodingLabelRequest) -> Result<MasterCut> {
    let assignment: BTreeMap<StableVariableId, i64> = request.assignment.iter()
        .map(|row| (StableVariableId(row.variable.clone()), row.value)).collect();
    let mut cut = cut;
    cut.encoding = match request.encoding.as_deref().unwrap_or("linear") {
        "linear" => MasterCutEncoding::Linear,
        "binaryNoGood" => MasterCutEncoding::BinaryNoGood { assignment },
        "boundedIntegerNoGood" => MasterCutEncoding::BoundedIntegerNoGood { assignment,
            family_id: request.family_id.clone().unwrap_or_default(),
            row: BoundedIntegerNoGoodRow::RequireDifference },
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "encoding label failed validation: unknown encoding `{other}`"))),
    };
    Ok(cut)
}

/// 走一遍编码标签。非法输入是**错误**。 / Walk the encoding label.
pub fn verify_encoding_label(request: &EncodingLabelRequest) -> Result<EncodingLabelReport> {
    for term in &request.terms {
        if term.variable.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "encoding label failed validation: variable IDs must not be blank"));
        }
    }
    let plain = cut(request, "cut");
    let new_label = label(&plain.encoding).to_owned();
    let claimed = claim(cut(request, "cut"), request)?;
    let claimed_label = label(&claimed.encoding).to_owned();
    let keys_agree = plain.expression_key() == claimed.expression_key();
    let cuts_are_equal = plain == claimed;
    let normalized = claimed.normalize();
    let (normalize_accepted, normalized_label, normalized_family_id) = match &normalized {
        Ok(value) => {
            let family_id: Option<String> = match &value.encoding {
                MasterCutEncoding::BoundedIntegerNoGood { family_id, .. } => {
                    Some(family_id.clone())
                }
                _ => None,
            };
            (true, label(&value.encoding).to_owned(), family_id)
        }
        Err(_) => (false, String::new(), None),
    };
    let findings = canonical_findings();
    Ok(EncodingLabelReport {
        new_label,
        claimed_label,
        keys_agree,
        cuts_are_equal,
        normalize_accepted,
        normalized_label,
        normalized_family_id,
        new_always_claims_linear: findings.new_always_claims_linear,
        key_ignores_the_encoding: findings.key_ignores_the_encoding,
        normalize_ignores_the_encoding: findings.normalize_ignores_the_encoding,
        equality_sees_the_encoding: findings.equality_sees_the_encoding,
    })
}

/// 四条规则属性的内部固定探针。 / The four rule properties measured by fixed internal probes.
struct Findings {
    new_always_claims_linear: bool,
    key_ignores_the_encoding: bool,
    normalize_ignores_the_encoding: bool,
    equality_sees_the_encoding: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let probe = EncodingLabelRequest { encoding: None, assignment: Vec::new(), family_id: None,
        terms: vec![EncodingTerm { variable: "x".to_owned(), coefficient: 1.0 }], rhs: 2.0 };
    let plain = cut(&probe, "probe");
    // 1) `new` 恒给线性。 / `new` always claims linear.
    let new_always_claims_linear = label(&plain.encoding) == "linear";
    // 2) key 不读编码：换成二元 no-good 后 key 不变。 
    // 2) The key ignores the encoding: switching to a binary no-good leaves the key unchanged.
    let mut binary = plain.clone();
    binary.encoding = MasterCutEncoding::BinaryNoGood {
        assignment: BTreeMap::from([(StableVariableId("x".to_owned()), 1i64)]) };
    let mut bounded = plain.clone();
    // 一条**故意不自洽**的声明：赋值空、族 ID 空白。 / A deliberately inconsistent claim: empty assignment, blank family.
    bounded.encoding = MasterCutEncoding::BoundedIntegerNoGood {
        assignment: BTreeMap::new(), family_id: String::new(),
        row: BoundedIntegerNoGoodRow::RequireDifference };
    let key_ignores_the_encoding = plain.expression_key() == binary.expression_key()
        && plain.expression_key() == bounded.expression_key();
    // 3) `normalize()` 不校验编码：那条不自洽的声明仍然通过。 
    // 3) `normalize()` does not validate the encoding: the inconsistent claim still passes.
    let normalize_ignores_the_encoding = {
        let mut inconsistent = plain.clone();
        inconsistent.encoding = MasterCutEncoding::BoundedIntegerNoGood {
            assignment: BTreeMap::new(), family_id: String::new(),
            row: BoundedIntegerNoGoodRow::RequireDifference };
        match inconsistent.normalize() {
            Ok(value) => matches!(value.encoding, MasterCutEncoding::BoundedIntegerNoGood {
                family_id: ref name, .. } if name.is_empty()),
            Err(_) => false,
        }
    };
    // 4) `==` 读编码：表达式相同、编码不同 → 不相等。 
    // 4) `==` sees the encoding: identical expressions with different encodings are not equal.
    let equality_sees_the_encoding = plain.expression_key() == binary.expression_key()
        && plain != binary && binary != bounded;
    Findings { new_always_claims_linear, key_ignores_the_encoding,
        normalize_ignores_the_encoding, equality_sees_the_encoding }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> EncodingLabelRequest {
        EncodingLabelRequest { encoding: Some("binaryNoGood".to_owned()),
            assignment: vec![EncodingAssignmentRow { variable: "x".to_owned(), value: 1 }],
            family_id: None,
            terms: vec![EncodingTerm { variable: "x".to_owned(), coefficient: 1.0 }], rhs: 2.0 }
    }

    #[test]
    fn new_always_claims_linear() {
        let report = verify_encoding_label(&request()).unwrap();
        assert_eq!(report.new_label, "linear");
        assert_eq!(report.claimed_label, "binaryNoGood");
        assert!(report.new_always_claims_linear);
    }

    #[test]
    fn the_key_ignores_the_encoding_but_equality_does_not() {
        let report = verify_encoding_label(&request()).unwrap();
        // 表达式相同 → key 相同。 / Identical expressions → identical keys.
        assert!(report.keys_agree);
        // 但 `==` 区分编码。 / But `==` distinguishes the encoding.
        assert!(!report.cuts_are_equal);
        assert!(report.key_ignores_the_encoding);
        assert!(report.equality_sees_the_encoding);
    }

    #[test]
    fn an_inconsistent_bounded_claim_still_normalizes() {
        let document = EncodingLabelRequest { encoding: Some("boundedIntegerNoGood".to_owned()),
            assignment: Vec::new(), family_id: Some("   ".to_owned()),
            terms: vec![EncodingTerm { variable: "x".to_owned(), coefficient: 1.0 }], rhs: 2.0 };
        let report = verify_encoding_label(&document).unwrap();
        assert_eq!(report.claimed_label, "boundedIntegerNoGood");
        // 空赋值 + 空白族 ID 的声明照样过 `normalize()`。 
        // An empty assignment with a blank family id still normalizes.
        assert!(report.normalize_accepted);
        assert_eq!(report.normalized_label, "boundedIntegerNoGood");
        // **按实测纠正**：族 ID **原样保留**（连空格一起）——`normalize()` 只 `trim` 证明引用，
        // 不碰 `family_id`，也不校验它非空。
        // Measured correction: the family id is kept **verbatim, spaces included** — `normalize()` only trims the proof
        // reference; it neither touches `family_id` nor checks that it is non-blank.
        assert_eq!(report.normalized_family_id.as_deref(), Some("   "));
        assert!(report.normalize_ignores_the_encoding);
    }

    /// **按实测纠正**：`PartialEq` 是**整结构**比较，所以想证明"看得见编码"必须让其它字段全同。
    /// Measured correction: `PartialEq` compares the **whole struct**, so proving "it sees the encoding" requires every
    /// other field to be identical.
    #[test]
    fn a_plain_linear_cut_is_still_equal_to_itself() {
        let mut document = request();
        document.encoding = Some("linear".to_owned());
        document.assignment = Vec::new();
        let report = verify_encoding_label(&document).unwrap();
        assert_eq!(report.claimed_label, "linear");
        assert!(report.keys_agree);
        // 编码相同 → 相等。 / Same encoding → equal.
        assert!(report.cuts_are_equal);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let report = verify_encoding_label(&EncodingLabelRequest {
            encoding: Some("linear".to_owned()), assignment: Vec::new(), family_id: None,
            terms: Vec::new(), rhs: 0.0 }).unwrap();
        assert!(report.new_always_claims_linear);
        assert!(report.key_ignores_the_encoding);
        assert!(report.normalize_ignores_the_encoding);
        assert!(report.equality_sees_the_encoding);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut unknown = request();
        unknown.encoding = Some("quadraticNoGood".to_owned());
        assert!(verify_encoding_label(&unknown).unwrap_err().message
            .contains("unknown encoding"));
        let mut blank = request();
        blank.terms[0].variable = "  ".to_owned();
        assert!(verify_encoding_label(&blank).unwrap_err().message
            .contains("variable IDs must not be blank"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_encoding_label(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"newLabel\""), "{json}");
        assert!(json.contains("\"keyIgnoresTheEncoding\""), "{json}");
        assert!(json.contains("\"normalizedFamilyId\""), "{json}");
        assert!(!json.contains("new_label"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<EncodingLabelReport>(&copy.to_string()).is_err());
    }
}

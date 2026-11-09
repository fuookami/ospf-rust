//! 续挖第 37 单：二进制 no-good 的极性编码，与有界整数家族三行的**系数/右端项闭式**
//! （`solver/logic_based_benders.rs` 的 `binary_assignment_no_good` + `MasterCutEncoding::BinaryNoGood`）
//! Continuation, delivery 37: the polarity encoding of a binary no-good plus the closed forms of the bounded-integer
//! family's rows (`binary_assignment_no_good`, `MasterCutEncoding::BinaryNoGood`).
//!
//! 第 33 单量了家族的**行数与辅助集**，这一单量每一行的**系数与右端项**——这些数字全是闭式，可以照着算：
//!
//! 1. **二进制 no-good 的系数就是"取反的位"**：`value == 1` → `-1.0`，`value == 0` → `+1.0`，
//!    关系方向恒为 `GreaterOrEqual`，右端项 = **`1 - 1 的个数`**。三变量全 1 时 rhs 是 **`-2`**（负数），
//!    它表达的是 `x1+x2+x3 <= 2`——"排除全 1 点"在这里是**不等于 3**，不是"不等于某个正数"。
//! 2. **cut ID 是 canonical key 的直接函数**：`logic/no-good/{name=value;...}`，按稳定 ID 升序拼接。
//!    所以 ID 只取决于"被排除的赋值"，不取决于证明引用、来源或原因。
//! 3. **`BinaryNoGood` 编码里存的是完整赋值**（每个变量都在），而系数里也每个变量都在——两者长度相等。
//! 4. **有界整数家族的两条边行，右端项就是值域边界**：`lowerSide` 的 rhs 等于该变量的**上界**，
//!    `upperSide` 的 rhs 等于**下界**；两行的辅助系数分别是 `+m_upper`（`upper - value + 1`）与
//!    `-m_lower`（`-(value + 1 - lower)`）。所以"边行选了哪一侧"这件事，从 rhs 与值域的关系就能读出来。
//! 5. **`AtMostOne` 与 `RequireDifference` 的系数全是 `1.0`**，rhs 都是 `1.0`；区别只在
//!    `AtMostOne` 用**两个**辅助变量（某个变量的 lower/upper），`RequireDifference` 用**全部 `2n` 个**。
//!
//! Delivery 33 measured the family's **row count and auxiliary set**; this one measures each row's **coefficients and
//! right-hand side**, all of which are closed forms. **A binary no-good's coefficients are the negated bits**: `value == 1`
//! gives `-1.0` and `value == 0` gives `+1.0`, the sense is always `GreaterOrEqual` and the right-hand side is **`1 - (number
//! of ones)`** — three variables all 1 give **`-2`**, which states `x1+x2+x3 <= 2`, so "exclude the all-ones point" is
//! "not equal to 3" rather than "not equal to some positive number". **The cut ID is a direct function of the canonical
//! key**: `logic/no-good/{name=value;...}` joined in ascending stable-ID order, so the ID depends only on the excluded
//! assignment and not on the proof reference, origin or reason. **The `BinaryNoGood` encoding stores the complete
//! assignment** — every variable appears both in the encoding and in the coefficients. And **the bounded family's two side
//! rows carry the domain bounds as their right-hand sides**: `lowerSide`'s rhs equals the variable's **upper** bound,
//! `upperSide`'s equals its **lower** bound, with auxiliary coefficients `+m_upper` (`upper - value + 1`) and `-m_lower`
//! (`-(value + 1 - lower)`) — so which side a row selects is readable from the rhs against the domain. Note also that
//! **`AtMostOne` and `RequireDifference` use unit coefficients and rhs 1.0**, differing only in scope: `AtMostOne` covers the
//! **two** auxiliaries of one variable while `RequireDifference` covers **all `2n`**.

use std::collections::{BTreeMap, BTreeSet};

use ospf_rust_core::solver::report::StableVariableId;
use ospf_rust_framework::solver::logic_based_benders::{
    binary_assignment_no_good, bounded_integer_assignment_no_good, BoundedIntegerNoGoodRow,
    MasterAssignment, MasterBinding, MasterCut, MasterCutEncoding, MasterVariableBinding,
};
use ospf_rust_framework::solver::CutSense;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个二元主问题变量。 / One binary master variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BinaryVariableSpec {
    /// 稳定变量 ID。 / The stable variable ID.
    pub id: String,
    /// 取值（必须是 0 或 1）。 / The value (must be 0 or 1).
    pub value: i64,
}

/// 一个整数主问题变量。 / One integer master variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntegerVariableSpec {
    /// 稳定变量 ID。 / The stable variable ID.
    pub id: String,
    /// 下界。 / The lower bound.
    pub lower: i64,
    /// 上界。 / The upper bound.
    pub upper: i64,
    /// 取值。 / The value.
    pub value: i64,
}

/// 极性编码请求。 / The polarity-encoding request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolarityRequest {
    /// 二进制赋值。 / The binary assignment.
    #[serde(default)]
    pub binary: Vec<BinaryVariableSpec>,
    /// 整数赋值（用于家族的三行闭式）。 / The integer assignment (for the family's row closed forms).
    #[serde(default)]
    pub integers: Vec<IntegerVariableSpec>,
    /// 证明引用。 / The proof reference.
    #[serde(default)]
    pub proof_reference: String,
}

/// 一个稀疏项。 / One sparse term.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SparseTerm {
    /// 变量 ID。 / The variable ID.
    pub variable: String,
    /// 系数。 / The coefficient.
    pub coefficient: f64,
}

/// 一行闭式。 / One row's closed form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RowClosedForm {
    /// cut ID。 / The cut ID.
    pub id: String,
    /// 稀疏系数。 / The sparse coefficients.
    pub terms: Vec<SparseTerm>,
    /// 右端项。 / The right-hand side.
    pub rhs: f64,
    /// 关系方向名。 / The sense name.
    pub sense: String,
}

/// 家族三行的闭式。 / The closed forms of three family rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FamilyRowForms {
    /// `lowerSide` 行。 / The `lowerSide` row.
    pub lower_side: RowClosedForm,
    /// `upperSide` 行。 / The `upperSide` row.
    pub upper_side: RowClosedForm,
    /// `atMostOne` 行。 / The `atMostOne` row.
    pub at_most_one: RowClosedForm,
    /// `requireDifference` 行。 / The `requireDifference` row.
    pub require_difference: RowClosedForm,
}

/// 极性编码结论。 / The polarity verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolarityReport {
    /// 二进制 no-good 的这一条 cut。 / The binary no-good cut.
    pub no_good: RowClosedForm,
    /// `BinaryNoGood` 编码里的赋值条数。 / The assignment count inside the `BinaryNoGood` encoding.
    pub encoding_count: u64,
    /// 1 的个数。 / The number of ones.
    pub ones: u64,
    /// 每个整数变量的家族行闭式。 / The family rows for each integer variable.
    pub families: Vec<FamilyRowForms>,
    /// 系数是取反的位（**是**）。 / Whether coefficients are the negated bits (**yes**).
    pub coefficients_are_the_negated_bits: bool,
    /// 右端项是 1 减 1 的个数（**是**）。 / Whether the rhs is 1 minus the ones count (**yes**).
    pub rhs_is_one_minus_the_ones: bool,
    /// 全 1 时右端项为负（**是**）。 / Whether an all-ones assignment gives a negative rhs (**yes**).
    pub all_ones_rhs_is_negative: bool,
    /// ID 只取决于被排除的赋值（**是**）。 / Whether the ID depends only on the excluded assignment (**yes**).
    pub id_is_a_pure_function_of_the_assignment: bool,
    /// 编码里存完整赋值（**是**）。 / Whether the encoding stores the complete assignment (**yes**).
    pub encoding_holds_every_variable: bool,
    /// 边行右端项就是值域边界（**是**）。 / Whether a side row's rhs is a domain bound (**yes**).
    pub side_rows_carry_domain_bounds: bool,
    /// `atMostOne` 只用两个辅助变量（**是**）。 / Whether `atMostOne` uses two auxiliaries (**yes**).
    pub at_most_one_is_a_pair: bool,
    /// `requireDifference` 覆盖全部 `2n`（**是**）。 / Whether `requireDifference` spans all `2n` (**yes**).
    pub require_difference_spans_every_auxiliary: bool,
    /// 两条无辅助身份的边行都用 Unit 系数（**是**）。 / Whether both unit-coefficient rows use rhs 1.0 (**yes**).
    pub unit_rows_use_rhs_one: bool,
}

/// 变量条数上限（信任边界）。 / The variable cap (a trust boundary).
const MAX_VARIABLES: usize = 32;

/// 关系方向名。 / The sense name.
fn sense_name(sense: CutSense) -> &'static str {
    match sense {
        CutSense::LessOrEqual => "lessOrEqual",
        CutSense::GreaterOrEqual => "greaterOrEqual",
        CutSense::Equal => "equal",
    }
}

/// 把一条 cut 读成闭式。 / Read one cut as a closed form.
fn form(cut: &MasterCut) -> RowClosedForm {
    RowClosedForm { id: cut.id.clone(), terms: cut.coefficients.iter()
        .map(|(variable, coefficient)| SparseTerm { variable: variable.0.clone(),
            coefficient: *coefficient }).collect(),
        rhs: cut.rhs, sense: sense_name(cut.sense).to_owned() }
}

/// 走一遍极性编码。非法输入是**错误**。 / Walk the polarity encoding.
pub fn verify_polarity_encoding(request: &PolarityRequest) -> Result<PolarityReport> {
    if request.binary.is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "polarity encoding failed validation: at least one binary variable is required"));
    }
    if request.binary.len() + request.integers.len() > MAX_VARIABLES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "polarity encoding failed validation: at most {MAX_VARIABLES} variables are accepted")));
    }
    if request.proof_reference.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "polarity encoding failed validation: the proof reference must not be blank"));
    }
    let mut ids = BTreeSet::new();
    let mut bindings = Vec::new();
    let mut values = BTreeMap::new();
    for variable in &request.binary {
        if variable.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "polarity encoding failed validation: variable IDs must not be blank"));
        }
        if !ids.insert(variable.id.as_str()) {
            return Err(BridgeError::new("InvalidInput",
                "polarity encoding failed validation: the request repeats a variable"));
        }
        bindings.push(MasterVariableBinding::binary(variable.id.as_str()));
        values.insert(StableVariableId(variable.id.clone()), variable.value);
    }
    let mut integer_ids = Vec::new();
    for variable in &request.integers {
        if variable.id.trim().is_empty() {
            return Err(BridgeError::new("InvalidInput",
                "polarity encoding failed validation: variable IDs must not be blank"));
        }
        if !ids.insert(variable.id.as_str()) {
            return Err(BridgeError::new("InvalidInput",
                "polarity encoding failed validation: the request repeats a variable"));
        }
        if variable.lower > variable.upper {
            return Err(BridgeError::new("InvalidInput",
                "polarity encoding failed validation: a lower bound exceeds its upper bound"));
        }
        bindings.push(MasterVariableBinding::integer(variable.id.as_str(), variable.lower,
            variable.upper));
        values.insert(StableVariableId(variable.id.clone()), variable.value);
        integer_ids.push(variable.id.clone());
    }
    let binding = MasterBinding::new(bindings)
        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
    let assignment = MasterAssignment::new(values, &binding)
        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
    let binary_ids: BTreeSet<StableVariableId> = request.binary.iter()
        .map(|variable| StableVariableId(variable.id.clone())).collect();
    let cut = binary_assignment_no_good(&assignment, &binary_ids, request.proof_reference.as_str())
        .map_err(|error| BridgeError::new("InvalidInput", &error))?;
    let encoding_count = match &cut.encoding {
        MasterCutEncoding::BinaryNoGood { assignment } => assignment.len() as u64,
        _ => 0,
    };
    let ones = request.binary.iter().filter(|variable| variable.value == 1).count() as u64;
    // 整数家族：每个变量各出一组三行 + 一条 require-difference。
    // The integer family: three rows per variable plus one require-difference.
    let mut families = Vec::with_capacity(integer_ids.len());
    if !integer_ids.is_empty() {
        let family_ids: BTreeSet<StableVariableId> = integer_ids.iter()
            .map(|id| StableVariableId(id.clone())).collect();
        let cuts = bounded_integer_assignment_no_good(&assignment, &family_ids, &binding,
            request.proof_reference.as_str())
            .map_err(|error| BridgeError::new("InvalidInput", &error))?;
        for id in &integer_ids {
            let pick = |which: &str| -> Option<RowClosedForm> {
                cuts.iter().find(|cut| match (&cut.encoding, which) {
                    (MasterCutEncoding::BoundedIntegerNoGood { row: BoundedIntegerNoGoodRow::LowerSide(candidate), .. }, "lower") =>
                        &candidate.0 == id,
                    (MasterCutEncoding::BoundedIntegerNoGood { row: BoundedIntegerNoGoodRow::UpperSide(candidate), .. }, "upper") =>
                        &candidate.0 == id,
                    (MasterCutEncoding::BoundedIntegerNoGood { row: BoundedIntegerNoGoodRow::AtMostOne(candidate), .. }, "atMostOne") =>
                        &candidate.0 == id,
                    (MasterCutEncoding::BoundedIntegerNoGood { row: BoundedIntegerNoGoodRow::RequireDifference, .. }, "requireDifference") =>
                        true,
                    _ => false,
                }).map(form)
            };
            let lower_side = pick("lower").ok_or_else(|| BridgeError::new("InternalInvariant",
                "the bounded family is missing a lower-side row"))?;
            let upper_side = pick("upper").ok_or_else(|| BridgeError::new("InternalInvariant",
                "the bounded family is missing an upper-side row"))?;
            let at_most_one = pick("atMostOne").ok_or_else(|| BridgeError::new("InternalInvariant",
                "the bounded family is missing an at-most-one row"))?;
            let require_difference = pick("requireDifference").ok_or_else(|| BridgeError::new(
                "InternalInvariant", "the bounded family is missing a require-difference row"))?;
            families.push(FamilyRowForms { lower_side, upper_side, at_most_one,
                require_difference });
        }
    }
    let findings = canonical_findings();
    Ok(PolarityReport { no_good: form(&cut), encoding_count, ones, families,
        coefficients_are_the_negated_bits: findings.coefficients_are_the_negated_bits,
        rhs_is_one_minus_the_ones: findings.rhs_is_one_minus_the_ones,
        all_ones_rhs_is_negative: findings.all_ones_rhs_is_negative,
        id_is_a_pure_function_of_the_assignment: findings.id_is_a_pure_function_of_the_assignment,
        encoding_holds_every_variable: findings.encoding_holds_every_variable,
        side_rows_carry_domain_bounds: findings.side_rows_carry_domain_bounds,
        at_most_one_is_a_pair: findings.at_most_one_is_a_pair,
        require_difference_spans_every_auxiliary: findings.require_difference_spans_every_auxiliary,
        unit_rows_use_rhs_one: findings.unit_rows_use_rhs_one })
}

/// 九条规则属性的内部固定探针。 / The nine rule properties measured by fixed internal probes.
struct Findings {
    coefficients_are_the_negated_bits: bool,
    rhs_is_one_minus_the_ones: bool,
    all_ones_rhs_is_negative: bool,
    id_is_a_pure_function_of_the_assignment: bool,
    encoding_holds_every_variable: bool,
    side_rows_carry_domain_bounds: bool,
    at_most_one_is_a_pair: bool,
    require_difference_spans_every_auxiliary: bool,
    unit_rows_use_rhs_one: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let binding = MasterBinding::new([
        MasterVariableBinding::binary("probe.a"),
        MasterVariableBinding::binary("probe.b"),
        MasterVariableBinding::binary("probe.c"),
        MasterVariableBinding::integer("probe.i", 0, 4),
        MasterVariableBinding::integer("probe.j", -2, 3),
    ]).expect("probe binding");
    let values: BTreeMap<StableVariableId, i64> = [
        (StableVariableId("probe.a".to_owned()), 1i64),
        (StableVariableId("probe.b".to_owned()), 0),
        (StableVariableId("probe.c".to_owned()), 1),
        (StableVariableId("probe.i".to_owned()), 2),
        (StableVariableId("probe.j".to_owned()), 0),
    ].into_iter().collect();
    let assignment = MasterAssignment::new(values, &binding).expect("probe assignment");
    let binary_ids: BTreeSet<StableVariableId> = ["probe.a", "probe.b", "probe.c"].iter()
        .map(|id| StableVariableId((*id).to_owned())).collect();
    let cut = binary_assignment_no_good(&assignment, &binary_ids, "probe/proof")
        .expect("probe no-good");
    let a = StableVariableId("probe.a".to_owned());
    let b = StableVariableId("probe.b".to_owned());
    let c = StableVariableId("probe.c".to_owned());
    // 1) 系数是取反的位。 / Coefficients are the negated bits.
    let coefficients_are_the_negated_bits = cut.coefficients.get(&a) == Some(&-1.0)
        && cut.coefficients.get(&b) == Some(&1.0)
        && cut.coefficients.get(&c) == Some(&-1.0)
        && cut.sense == CutSense::GreaterOrEqual;
    // 2) rhs = 1 - 1 的个数 = 1 - 2 = -1。 / The rhs is 1 minus the ones.
    let rhs_is_one_minus_the_ones = cut.rhs == -1.0;
    // 3) 全 1 → rhs 为负。 / All ones give a negative rhs.
    // **按实测补一条**：`MasterAssignment::new` 要求赋值**恰好覆盖**绑定集，所以这里不能只给三个二元变量，
    // 整数变量也必须一起给——否则报 "master assignment does not match the stable binding set"。
    // Measured note: `MasterAssignment::new` requires the assignment to cover the binding set **exactly**, so the integer
    // variables must be included too; otherwise it reports "master assignment does not match the stable binding set".
    let all_ones_values: BTreeMap<StableVariableId, i64> = [
        (a.clone(), 1i64), (b.clone(), 1), (c.clone(), 1),
        (StableVariableId("probe.i".to_owned()), 1),
        (StableVariableId("probe.j".to_owned()), 1),
    ].into_iter().collect();
    let all_ones = MasterAssignment::new(all_ones_values, &binding).expect("probe assignment");
    let all_ones_cut = binary_assignment_no_good(&all_ones, &binary_ids, "probe/proof")
        .expect("probe no-good");
    let all_ones_rhs_is_negative = all_ones_cut.rhs < 0.0 && all_ones_cut.rhs == -2.0;
    // 4) ID 只取决于被排除的赋值：换证明引用与来源后 ID 不变。 
    // 4) The ID depends only on the excluded assignment: another proof reference keeps it.
    let other = binary_assignment_no_good(&assignment, &binary_ids, "another/proof")
        .expect("probe no-good");
    // **按实测纠正**：ID 里的变量名是**转义过的**（`b.a` → `3:622e61`，非字母数字走十六进制），
    // 所以不能直接用原名字面量去查；只比较"换证明引用后 ID 不变"这件事本身。
    // Measured correction: variable names inside the ID are **escaped** (`b.a` becomes `3:622e61`, hex for non-alphanumerics),
    // so the raw name cannot be searched for; only "the ID survives another proof reference" is asserted.
    let id_is_a_pure_function_of_the_assignment = other.id == cut.id
        && cut.id.starts_with("logic/no-good/") && cut.id.contains('=');
    // 5) 编码里存完整赋值。 / The encoding stores the complete assignment.
    let encoding_holds_every_variable = matches!(&cut.encoding,
        MasterCutEncoding::BinaryNoGood { assignment } if assignment.len() == 3);
    // 6-9) 家族三行的闭式。 / The family rows' closed forms.
    let family_ids: BTreeSet<StableVariableId> = ["probe.i", "probe.j"].iter()
        .map(|id| StableVariableId((*id).to_owned())).collect();
    let cuts = bounded_integer_assignment_no_good(&assignment, &family_ids, &binding,
        "probe/proof").expect("probe family");
    let aux_of = |cut: &MasterCut, master: &str| -> Option<f64> {
        cut.coefficients.iter().find(|(id, _)| id.0 != master).map(|(_, value)| *value)
    };
    let find = |master: &str, kind: &str| cuts.iter().find(|cut| match (&cut.encoding, kind) {
        (MasterCutEncoding::BoundedIntegerNoGood { row: BoundedIntegerNoGoodRow::LowerSide(id), .. }, "lower") => id.0 == master,
        (MasterCutEncoding::BoundedIntegerNoGood { row: BoundedIntegerNoGoodRow::UpperSide(id), .. }, "upper") => id.0 == master,
        (MasterCutEncoding::BoundedIntegerNoGood { row: BoundedIntegerNoGoodRow::AtMostOne(id), .. }, "atMostOne") => id.0 == master,
        _ => false,
    });
    // probe.i：域 [0,4]、值 2 → lower 行 rhs = 上界 4、辅助系数 = 4 - 2 + 1 = 3；upper 行 rhs = 下界 0、辅助系数 = -3。
    // probe.i: domain [0,4] at value 2 → the lower row's rhs is the upper bound 4 with auxiliary 3; the upper row's rhs
    // is the lower bound 0 with auxiliary -3.
    let i_lower = find("probe.i", "lower").map(|cut| (cut.rhs, aux_of(cut, "probe.i")));
    let i_upper = find("probe.i", "upper").map(|cut| (cut.rhs, aux_of(cut, "probe.i")));
    // probe.j：域 [-2,3]、值 0 → lower 行 rhs = 3、辅助 4；upper 行 rhs = -2、辅助 -1。
    // probe.j: domain [-2,3] at value 0 → the lower row's rhs is 3 with auxiliary 4; the upper row's rhs is -2 with -1.
    let j_lower = find("probe.j", "lower").map(|cut| (cut.rhs, aux_of(cut, "probe.j")));
    let j_upper = find("probe.j", "upper").map(|cut| (cut.rhs, aux_of(cut, "probe.j")));
    // **按实测纠正**：probe.j 的 upper 行辅助系数是 `-m_lower = -((0+1) - (-2)) = -3`，不是我原来写的 -1
    // （我把 `value + 1 - lower` 的结果当成了系数，而系数是它的**相反数**）。
    // Measured correction: `probe.j`'s upper row carries `-m_lower = -((0+1) - (-2)) = -3`, not the -1 I first wrote — I
    // mistook `value + 1 - lower` for the coefficient, which is its negation.
    let side_rows_carry_domain_bounds = i_lower == Some((4.0, Some(3.0)))
        && i_upper == Some((0.0, Some(-3.0)))
        && j_lower == Some((3.0, Some(4.0)))
        && j_upper == Some((-2.0, Some(-3.0)));
    let at_most_one_is_a_pair = find("probe.i", "atMostOne").is_some_and(|cut| {
        cut.coefficients.len() == 2 && cut.rhs == 1.0
            && cut.coefficients.values().all(|value| *value == 1.0)
    });
    // 家族 2 个变量 → 7 行；require-difference 系数覆盖全部 4 个辅助变量。
    // Two family variables give seven rows; require-difference spans all four auxiliaries.
    let require_difference_spans_every_auxiliary = cuts.len() == 7
        && cuts.iter().find(|cut| matches!(&cut.encoding,
            MasterCutEncoding::BoundedIntegerNoGood { row: BoundedIntegerNoGoodRow::RequireDifference, .. }))
            .is_some_and(|cut| cut.coefficients.len() == 4
                && cut.coefficients.values().all(|value| *value == 1.0)
                && cut.rhs == 1.0 && cut.sense == CutSense::GreaterOrEqual);
    let unit_rows_use_rhs_one = cuts.iter().filter(|cut| matches!(&cut.encoding,
        MasterCutEncoding::BoundedIntegerNoGood { row: BoundedIntegerNoGoodRow::AtMostOne(_)
            | BoundedIntegerNoGoodRow::RequireDifference, .. }))
        .all(|cut| cut.rhs == 1.0);
    Findings { coefficients_are_the_negated_bits, rhs_is_one_minus_the_ones,
        all_ones_rhs_is_negative, id_is_a_pure_function_of_the_assignment,
        encoding_holds_every_variable, side_rows_carry_domain_bounds, at_most_one_is_a_pair,
        require_difference_spans_every_auxiliary, unit_rows_use_rhs_one }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> PolarityRequest {
        PolarityRequest {
            binary: vec![
                BinaryVariableSpec { id: "b.a".to_owned(), value: 1 },
                BinaryVariableSpec { id: "b.b".to_owned(), value: 0 },
                BinaryVariableSpec { id: "b.c".to_owned(), value: 1 },
            ],
            integers: vec![IntegerVariableSpec { id: "i.x".to_owned(), lower: 0, upper: 4,
                value: 2 }],
            proof_reference: "probe/proof".to_owned(),
        }
    }

    #[test]
    fn the_binary_no_good_encodes_the_negated_bits() {
        let report = verify_polarity_encoding(&request()).unwrap();
        assert_eq!(report.ones, 2);
        // 系数：值为 1 → -1，值为 0 → +1；rhs = 1 - 2 = -1。 
        // Coefficients: 1 becomes -1 and 0 becomes +1; the rhs is 1 - 2.
        let by = |variable: &str| report.no_good.terms.iter()
            .find(|term| term.variable == variable).map(|term| term.coefficient).unwrap();
        assert_eq!(by("b.a"), -1.0);
        assert_eq!(by("b.b"), 1.0);
        assert_eq!(by("b.c"), -1.0);
        assert_eq!(report.no_good.rhs, -1.0);
        assert_eq!(report.no_good.sense, "greaterOrEqual");
        assert_eq!(report.encoding_count, 3);
        assert!(report.no_good.id.starts_with("logic/no-good/"));
        assert!(report.coefficients_are_the_negated_bits);
        assert!(report.rhs_is_one_minus_the_ones);
        assert!(report.all_ones_rhs_is_negative);
        assert!(report.id_is_a_pure_function_of_the_assignment);
        assert!(report.encoding_holds_every_variable);
    }

    #[test]
    fn the_side_rows_carry_the_domain_bounds() {
        let report = verify_polarity_encoding(&request()).unwrap();
        let family = &report.families[0];
        // i.x：域 [0,4]、值 2 → lower rhs 4（上界）、upper rhs 0（下界）。 
        // i.x: domain [0,4] at value 2 → the lower row's rhs is 4 (the upper bound) and the upper row's is 0.
        assert_eq!(family.lower_side.rhs, 4.0);
        assert_eq!(family.lower_side.sense, "lessOrEqual");
        assert_eq!(family.upper_side.rhs, 0.0);
        assert_eq!(family.upper_side.sense, "greaterOrEqual");
        // 主变量系数恒为 1，另一项是 big-M。 / The master coefficient is 1 and the other term is the big-M.
        let master = family.lower_side.terms.iter()
            .find(|term| term.variable == "i.x").map(|term| term.coefficient).unwrap();
        assert_eq!(master, 1.0);
        let aux = family.lower_side.terms.iter()
            .find(|term| term.variable != "i.x").map(|term| term.coefficient).unwrap();
        // 4 - 2 + 1 = 3。 / 4 - 2 + 1 = 3.
        assert_eq!(aux, 3.0);
        assert!(report.side_rows_carry_domain_bounds);
    }

    #[test]
    fn the_unit_rows_use_rhs_one() {
        let report = verify_polarity_encoding(&request()).unwrap();
        let family = &report.families[0];
        assert_eq!(family.at_most_one.rhs, 1.0);
        assert_eq!(family.at_most_one.terms.len(), 2);
        assert!(family.at_most_one.terms.iter().all(|term| term.coefficient == 1.0));
        assert_eq!(family.require_difference.rhs, 1.0);
        assert_eq!(family.require_difference.sense, "greaterOrEqual");
        // 单变量家族：require-difference 有 2 个辅助项。 / One family variable: two auxiliaries.
        assert_eq!(family.require_difference.terms.len(), 2);
        assert!(report.at_most_one_is_a_pair);
        assert!(report.require_difference_spans_every_auxiliary);
        assert!(report.unit_rows_use_rhs_one);
    }

    #[test]
    fn the_rule_properties_hold_regardless_of_the_request() {
        let mut other = request();
        other.binary = vec![BinaryVariableSpec { id: "only".to_owned(), value: 0 }];
        other.integers = vec![IntegerVariableSpec { id: "n".to_owned(), lower: -2, upper: 3,
            value: 0 }];
        let report = verify_polarity_encoding(&other).unwrap();
        // 单变量、值为 0：系数 +1、rhs 1 - 0 = 1。 / One variable at 0: coefficient +1 and rhs 1.
        assert_eq!(report.no_good.rhs, 1.0);
        assert_eq!(report.no_good.terms[0].coefficient, 1.0);
        assert_eq!(report.ones, 0);
        assert!(report.coefficients_are_the_negated_bits);
        assert!(report.rhs_is_one_minus_the_ones);
        assert!(report.all_ones_rhs_is_negative);
        assert!(report.id_is_a_pure_function_of_the_assignment);
        assert!(report.encoding_holds_every_variable);
        assert!(report.side_rows_carry_domain_bounds);
        assert!(report.at_most_one_is_a_pair);
        assert!(report.require_difference_spans_every_auxiliary);
        assert!(report.unit_rows_use_rhs_one);
        // n：域 [-2,3]、值 0 → lower rhs 3、upper rhs -2。 / n: [-2,3] at 0 → lower rhs 3, upper rhs -2.
        assert_eq!(report.families[0].lower_side.rhs, 3.0);
        assert_eq!(report.families[0].upper_side.rhs, -2.0);
    }

    #[test]
    fn bad_input_is_refused() {
        let mut empty = request();
        empty.binary = Vec::new();
        assert!(verify_polarity_encoding(&empty).unwrap_err().message
            .contains("at least one binary variable"));
        let mut blank_proof = request();
        blank_proof.proof_reference = "  ".to_owned();
        assert!(verify_polarity_encoding(&blank_proof).unwrap_err().message
            .contains("proof reference must not be blank"));
        let mut duplicate = request();
        duplicate.binary[1].id = "b.a".to_owned();
        assert!(verify_polarity_encoding(&duplicate).unwrap_err().message
            .contains("repeats a variable"));
        let mut across = request();
        across.integers[0].id = "b.a".to_owned();
        assert!(verify_polarity_encoding(&across).unwrap_err().message
            .contains("repeats a variable"));
        let mut bad_binary = request();
        bad_binary.binary[0].value = 2;
        assert!(verify_polarity_encoding(&bad_binary).is_err());
        let mut out_of_domain = request();
        out_of_domain.integers[0].value = 9;
        assert!(verify_polarity_encoding(&out_of_domain).is_err());
        let mut inverted = request();
        inverted.integers[0].lower = 5;
        inverted.integers[0].upper = 1;
        assert!(verify_polarity_encoding(&inverted).unwrap_err().message
            .contains("lower bound exceeds its upper bound"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_polarity_encoding(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"coefficientsAreTheNegatedBits\""), "{json}");
        assert!(json.contains("\"rhsIsOneMinusTheOnes\""), "{json}");
        assert!(json.contains("\"lowerSide\""), "{json}");
        assert!(!json.contains("lower_side"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<PolarityReport>(&copy.to_string()).is_err());
    }
}

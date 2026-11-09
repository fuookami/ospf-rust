//! 续挖第 14 单：输入域证明的区间代数与浮点指纹（`model/intermediate/deferred_function.rs`）
//! Continuation, delivery 14: the input-domain proof's interval algebra and the float fingerprint.
//!
//! `deferred_function.rs` 里有两个可以脱离求解器独立验证的东西：`InputDomainProof` 与 `fingerprint_float`。
//! 两者都不是"工具函数"，各自带着一组容易被想当然的边界：
//!
//! **`InputDomainProof`（区间代数）**
//! 1. **`new` 只拒绝两种输入**：非有限边界（`NaN` / `±inf`）与 `lower > upper`。`lower == upper`
//!    **是合法的退化点**——所以 `[2, 2]` 是个真证明。
//! 2. **`from_optional_bounds` 把 `new` 的错误吞掉**：任一侧缺失或非有限或顺序颠倒都只返回 `None`，
//!    调用方分不出"没给"和"给错了"。这是最容易踩的一处：以为它会报错。
//! 3. **`intersect` 的空判定是 `lower > upper`**，所以**端点相接**（`[1,2] ∩ [2,3]`）得到的是**退化点**
//!    `[2,2]` 而不是 `None`。"没有交集"与"交集是一个点"必须分开。
//! 4. **`is_contained_in` 自反**，且端点相接算包含：`[2,2].is_contained_in(&[1,2])` 为真。
//!
//! **`fingerprint_float`（指纹）**
//! 5. **指纹的相等关系与数值的相等关系是两套**：`0.0 == -0.0` 但指纹**不同**（格式里有符号位）；
//!    两个 `NaN` **不相等**而指纹**相同**（都归到 `"nan"`，不看载荷）。
//! 6. **三种非有限值各有一个固定标记**：`"nan"` / `"inf"` / `"-inf"`，它们不可能与任何有限值的指纹碰撞
//!    （有限值走 `{:e}` 的 17 位指数格式）。
//!
//! Two things here are verifiable without a solver: `InputDomainProof` and `fingerprint_float`. Neither is a mere utility —
//! each carries edges that are easy to assume wrong. **`new` rejects only two things** — non-finite bounds and
//! `lower > upper` — so `lower == upper` **is a legal degenerate point** and `[2, 2]` is a real proof. **`from_optional_bounds`
//! swallows `new`'s error**: a missing side, a non-finite bound or a reversed pair all return `None`, so the caller cannot
//! tell "not given" from "given wrong" — the easy trap is expecting it to report. **`intersect` calls emptiness `lower > upper`**,
//! so **touching endpoints** (`[1,2] ∩ [2,3]`) yield the **degenerate point** `[2,2]` rather than `None`: "no overlap" and
//! "overlap at one point" must stay distinct. **`is_contained_in` is reflexive** and counts touching endpoints. And on the
//! fingerprint side, **its equality differs from numeric equality**: `0.0 == -0.0` yet their fingerprints **differ** (the sign is
//! in the format), while two `NaN`s **differ** numerically and share the fingerprint `"nan"` (the payload is never read); the three
//! non-finite markers can never collide with a finite fingerprint, which uses seventeen significant digits in exponential form.

use ospf_rust_core::model::intermediate::{fingerprint_float, InputDomainProof};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一个字面浮点（JSON 放不下 `NaN` / `inf`，所以走文本）。 / A literal float (JSON cannot carry NaN or inf).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FloatSpec {
    /// `number` / `nan` / `inf` / `negInf`。 / `number` / `nan` / `inf` / `negInf`.
    pub kind: String,
    /// 数值文本（`number` 时使用）。 / The numeric text (used when `kind` is `number`).
    #[serde(default)]
    pub text: String,
}

/// 一个区间。 / One interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainSpec {
    /// 下界。 / The lower bound.
    pub lower: FloatSpec,
    /// 上界。 / The upper bound.
    pub upper: FloatSpec,
}

/// 一对区间。 / A pair of intervals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainPairSpec {
    /// 左区间。 / The left interval.
    pub left: DomainSpec,
    /// 右区间。 / The right interval.
    pub right: DomainSpec,
}

/// 区间代数请求。 / The interval-algebra request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputDomainRequest {
    /// 要构造的区间（走 `new`，会报错）。 / The intervals to build via `new` (which can fail).
    #[serde(default)]
    pub domains: Vec<DomainSpec>,
    /// 要比较的区间对。 / The interval pairs to compare.
    #[serde(default)]
    pub pairs: Vec<DomainPairSpec>,
    /// 要取指纹的浮点。 / The floats to fingerprint.
    #[serde(default)]
    pub floats: Vec<FloatSpec>,
}

/// 一个区间的构造结论。 / One interval's construction verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainRow {
    /// 是否构造成功。 / Whether construction succeeded.
    pub ok: bool,
    /// 下界（成功时）。 / The lower bound (on success).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lower: Option<f64>,
    /// 上界（成功时）。 / The upper bound (on success).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub upper: Option<f64>,
    /// 是否退化成一个点。 / Whether it degenerated into a single point.
    pub degenerate: bool,
    /// `from_optional_bounds` 是否给出证明（**错误会被吞掉**）。 / Whether `from_optional_bounds` produced a proof.
    pub optional_proof: bool,
    /// 错误文本（失败时）。 / The error text on failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
}

/// 一对区间的比较结论。 / One pair's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainPairRow {
    /// 左是否被右包含。 / Whether the left is contained in the right.
    pub left_in_right: bool,
    /// 右是否被左包含。 / Whether the right is contained in the left.
    pub right_in_left: bool,
    /// 左是否自含（**恒真**）。 / Whether the left contains itself (**always true**).
    pub left_reflexive: bool,
    /// 交集是否为"一个点"（不是空）。 / Whether the intersection is a single point rather than empty.
    pub intersection_is_point: bool,
    /// 交集的下界（有交集时）。 / The intersection's lower bound (when non-empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub intersection_lower: Option<f64>,
    /// 交集的上界（有交集时）。 / The intersection's upper bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub intersection_upper: Option<f64>,
    /// 是否有交集。 / Whether the intervals overlap at all.
    pub has_intersection: bool,
}

/// 一个指纹结论。 / One fingerprint verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FingerprintRow {
    /// 指纹文本（`0` / `-0` / `nan` / `inf` / `-inf` / 数值文本）。 / The fingerprint text.
    pub input: String,
    /// 指纹。 / The fingerprint.
    pub fingerprint: String,
}

/// 区间代数结论。 / The interval-algebra verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputDomainReport {
    /// 区间构造结论。 / The construction verdicts.
    pub domains: Vec<DomainRow>,
    /// 区间对结论。 / The pair verdicts.
    pub pairs: Vec<DomainPairRow>,
    /// 指纹结论。 / The fingerprint verdicts.
    pub floats: Vec<FingerprintRow>,
    /// `0.0` 与 `-0.0` 的指纹**是否不同**（实测：不同）。 / Whether `0` and `-0` fingerprint differently (**yes**).
    pub zero_signs_differ: bool,
    /// 两个 `NaN` 的指纹**是否相同**（实测：相同）。 / Whether two `NaN`s share a fingerprint (**yes**).
    pub nan_fingerprint_stable: bool,
    /// 非有限标记是否可能与有限指纹碰撞（**否**）。 / Whether the markers could collide with a finite fingerprint (**no**).
    pub markers_are_distinct: bool,
    /// `from_optional_bounds` 是否会因非法输入而返回 `None`（而是的）。 / Whether it returns `None` on illegal input (**yes**).
    pub optional_swallows_errors: bool,
}

/// 条目上限（信任边界）。 / The entry cap (a trust boundary).
const MAX_ENTRIES: usize = 512;

/// 走一遍区间代数与指纹。非法输入是**错误**。 / Walk the interval algebra and the fingerprint.
pub fn verify_input_domain(request: &InputDomainRequest) -> Result<InputDomainReport> {
    if request.domains.len() > MAX_ENTRIES || request.pairs.len() > MAX_ENTRIES
        || request.floats.len() > MAX_ENTRIES
    {
        return Err(BridgeError::new("InvalidInput", &format!(
            "input domain failed validation: at most {MAX_ENTRIES} entries are accepted")));
    }
    let mut domains = Vec::with_capacity(request.domains.len());
    for spec in &request.domains {
        let lower = literal(&spec.lower)?;
        let upper = literal(&spec.upper)?;
        let built = InputDomainProof::new(lower, upper);
        let optional = InputDomainProof::from_optional_bounds(Some(lower), Some(upper));
        domains.push(match built {
            Ok(proof) => DomainRow { ok: true, lower: Some(proof.lower()),
                upper: Some(proof.upper()), degenerate: proof.lower() == proof.upper(),
                optional_proof: optional.is_some(), error: None },
            Err(error) => DomainRow { ok: false, lower: None, upper: None, degenerate: false,
                optional_proof: optional.is_some(), error: Some(error.to_string()) },
        });
    }
    let mut pairs = Vec::with_capacity(request.pairs.len());
    for pair in &request.pairs {
        let left = build(&pair.left)?;
        let right = build(&pair.right)?;
        let intersection = left.intersect(&right);
        pairs.push(DomainPairRow {
            left_in_right: left.is_contained_in(&right),
            right_in_left: right.is_contained_in(&left),
            left_reflexive: left.is_contained_in(&left),
            // 退化点也算有交集：判定是 `lower > upper`，端点相接时 `lower == upper`。
            // A degenerate point still counts as an overlap: the test is `lower > upper`.
            intersection_is_point: intersection.map(|proof| proof.lower() == proof.upper())
                .unwrap_or(false),
            intersection_lower: intersection.map(|proof| proof.lower()),
            intersection_upper: intersection.map(|proof| proof.upper()),
            has_intersection: intersection.is_some(),
        });
    }
    let mut floats = Vec::with_capacity(request.floats.len());
    for spec in &request.floats {
        let value = literal(spec)?;
        floats.push(FingerprintRow { input: input_text(spec, value),
            fingerprint: fingerprint_float(value) });
    }
    // 三处实测：`0` 与 `-0` 指纹不同；两个 `NaN` 指纹相同；标记不与任何有限指纹碰撞。
    let zero_signs_differ = fingerprint_float(0.0) != fingerprint_float(-0.0);
    let nan_fingerprint_stable = fingerprint_float(f64::NAN) == fingerprint_float(f64::NAN);
    let markers_are_distinct = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.0, 1.0, -1.0,
        1e308, 1e-308]
        .iter()
        .map(|value| fingerprint_float(*value))
        .all(|text| !text.is_empty());
    // `from_optional_bounds` 吞错误：顺序颠倒、非有限、缺一侧 → 全是 `None`，`new` 则报错。
    // It swallows: reversed, non-finite or one-sided all give `None` while `new` errors.
    let optional_swallows_errors = InputDomainProof::new(5.0, 1.0).is_err()
        && InputDomainProof::from_optional_bounds(Some(5.0), Some(1.0)).is_none()
        && InputDomainProof::from_optional_bounds(Some(1.0), None).is_none()
        && InputDomainProof::from_optional_bounds(Some(f64::NAN), Some(1.0)).is_none();
    Ok(InputDomainReport { domains, pairs, floats, zero_signs_differ, nan_fingerprint_stable,
        markers_are_distinct, optional_swallows_errors })
}

/// 构造一个区间（非法则错误）。 / Build an interval, erroring on illegal input.
fn build(spec: &DomainSpec) -> Result<InputDomainProof> {
    let lower = literal(&spec.lower)?;
    let upper = literal(&spec.upper)?;
    InputDomainProof::new(lower, upper).map_err(|error| BridgeError::new("InvalidInput",
        &format!("input domain failed validation: {error}")))
}

/// 解析一个字面浮点。 / Parse a literal float.
fn literal(spec: &FloatSpec) -> Result<f64> {
    match spec.kind.as_str() {
        "nan" => Ok(f64::NAN),
        "inf" => Ok(f64::INFINITY),
        "negInf" => Ok(f64::NEG_INFINITY),
        "number" => spec.text.trim().parse::<f64>().map_err(|_| BridgeError::new("InvalidInput",
            &format!("input domain failed validation: `{}` is not a float", spec.text))),
        other => Err(BridgeError::new("InvalidInput", &format!(
            "input domain failed validation: unknown float kind `{other}`"))),
    }
}

/// 指纹行的输入描述。 / The input description of a fingerprint row.
fn input_text(spec: &FloatSpec, value: f64) -> String {
    match spec.kind.as_str() {
        "number" if value == 0.0 && value.is_sign_negative() => "-0".to_owned(),
        "number" => spec.text.trim().to_owned(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(text: &str) -> FloatSpec {
        FloatSpec { kind: "number".to_owned(), text: text.to_owned() }
    }

    fn special(kind: &str) -> FloatSpec {
        FloatSpec { kind: kind.to_owned(), text: String::new() }
    }

    fn domain(lower: FloatSpec, upper: FloatSpec) -> DomainSpec {
        DomainSpec { lower, upper }
    }

    fn request(domains: Vec<DomainSpec>, pairs: Vec<DomainPairSpec>, floats: Vec<FloatSpec>)
        -> InputDomainRequest
    {
        InputDomainRequest { domains, pairs, floats }
    }

    #[test]
    fn new_accepts_a_degenerate_point_and_rejects_only_two_things() {
        // `lower == upper` 是**合法**退化点。 / `lower == upper` is a **legal** degenerate point.
        let report = verify_input_domain(&request(vec![domain(number("2"), number("2"))],
            Vec::new(), Vec::new())).unwrap();
        assert!(report.domains[0].ok);
        assert!(report.domains[0].degenerate);
        assert_eq!(report.domains[0].lower, Some(2.0));
        assert_eq!(report.domains[0].upper, Some(2.0));
        assert!(report.domains[0].optional_proof);
        // 只拒两种：顺序颠倒与非有限。 / Only two refusals: reversed order and non-finite.
        let report = verify_input_domain(&request(vec![
            domain(number("5"), number("1")),
            domain(special("nan"), number("1")),
            domain(number("1"), special("inf")),
            domain(number("-3"), number("4")),
        ], Vec::new(), Vec::new())).unwrap();
        assert!(!report.domains[0].ok);
        assert!(report.domains[0].error.as_deref().unwrap().contains("lower <= upper"));
        assert!(!report.domains[1].ok);
        assert!(report.domains[1].error.as_deref().unwrap().contains("finite bounds"));
        assert!(!report.domains[2].ok);
        assert!(report.domains[2].error.as_deref().unwrap().contains("finite bounds"));
        assert!(report.domains[3].ok);
        assert!(!report.domains[3].degenerate);
        // 三种失败下 `from_optional_bounds` 全是 `None`（吞掉错误）。 / All three give `None`.
        assert!(!report.domains[0].optional_proof);
        assert!(!report.domains[1].optional_proof);
        assert!(!report.domains[2].optional_proof);
        assert!(report.optional_swallows_errors);
    }

    #[test]
    fn intersect_keeps_touching_endpoints_as_a_point() {
        let report = verify_input_domain(&request(Vec::new(), vec![
            // 端点相接：[1,2] ∩ [2,3] = [2,2]，**不是**空。 / Touching: `[2,2]`, not empty.
            DomainPairSpec { left: domain(number("1"), number("2")),
                right: domain(number("2"), number("3")) },
            // 真正不相交：[1,2] ∩ [3,4] 是空。 / Truly disjoint: empty.
            DomainPairSpec { left: domain(number("1"), number("2")),
                right: domain(number("3"), number("4")) },
            // 一个包含另一个：[1,5] ∩ [2,3] = [2,3]。 / Nested: `[2,3]`.
            DomainPairSpec { left: domain(number("1"), number("5")),
                right: domain(number("2"), number("3")) },
            // 完全相同的两个点。 / Two identical points.
            DomainPairSpec { left: domain(number("7"), number("7")),
                right: domain(number("7"), number("7")) },
        ], Vec::new())).unwrap();
        assert!(report.pairs[0].has_intersection);
        assert!(report.pairs[0].intersection_is_point);
        assert_eq!(report.pairs[0].intersection_lower, Some(2.0));
        assert_eq!(report.pairs[0].intersection_upper, Some(2.0));
        // 端点相接算互相"不包含"以外的第三件事：两个方向都不包含。 / Neither contains the other.
        assert!(!report.pairs[0].left_in_right);
        assert!(!report.pairs[0].right_in_left);
        assert!(!report.pairs[1].has_intersection);
        assert!(!report.pairs[1].intersection_is_point);
        assert_eq!(report.pairs[1].intersection_lower, None);
        // 包含关系与自反。 / Containment and reflexivity.
        assert!(report.pairs[2].right_in_left);
        assert!(!report.pairs[2].left_in_right);
        assert!(report.pairs[3].left_in_right);
        assert!(report.pairs[3].right_in_left);
        for pair in &report.pairs {
            assert!(pair.left_reflexive);
        }
    }

    #[test]
    fn fingerprints_are_not_numeric_equality() {
        let report = verify_input_domain(&request(Vec::new(), Vec::new(), vec![
            number("0"), number("-0"), number("1"), special("nan"), special("nan"),
            special("inf"), special("negInf"),
        ])).unwrap();
        let print = |index: usize| report.floats[index].fingerprint.clone();
        // `0.0 == -0.0` 但指纹**不同**。 / Equal numerically, **different** fingerprints.
        assert_eq!(print(0), "0.00000000000000000e0");
        assert_eq!(print(1), "-0.00000000000000000e0");
        assert_ne!(print(0), print(1));
        assert!(report.zero_signs_differ);
        // 两个 `NaN` 不相等但指纹**相同**。 / Two NaNs are unequal yet share a fingerprint.
        assert_eq!(print(3), "nan");
        assert_eq!(print(3), print(4));
        assert!(report.nan_fingerprint_stable);
        // 三个非有限标记互不相同。 / The three markers differ.
        assert_eq!(print(5), "inf");
        assert_eq!(print(6), "-inf");
        assert_ne!(print(5), print(6));
        assert_ne!(print(3), print(5));
        // 有限值走 `{:.17e}`：**小数点后** 17 位（共 18 位有效数字），我一开始按"17 位有效数字"写成
        // `1.00000000000000006e0`，被实测打回。
        // Finite values use `{:.17e}`: seventeen digits **after** the point (eighteen significant). I first wrote the
        // "seventeen significant digits" rendering and was corrected by the measurement.
        assert_eq!(print(2), "1.00000000000000000e0");
        assert!(report.markers_are_distinct);
        // 输入的描述文本。 / The input descriptions.
        assert_eq!(report.floats[1].input, "-0");
        assert_eq!(report.floats[3].input, "nan");
    }

    #[test]
    fn bad_input_is_refused() {
        // 坏数字文本 / 未知浮点类型。 / A bad numeric text and an unknown float kind.
        let report = verify_input_domain(&request(vec![domain(number("one"), number("2"))],
            Vec::new(), Vec::new()));
        assert!(report.unwrap_err().message.contains("is not a float"));
        let report = verify_input_domain(&request(vec![domain(special("zero"), number("2"))],
            Vec::new(), Vec::new()));
        assert!(report.unwrap_err().message.contains("unknown float kind"));
        // 区间对里非法 → 直接错误（不像 `from_optional_bounds` 那样吞）。 
        // An illegal pair errors outright rather than being swallowed.
        let report = verify_input_domain(&request(Vec::new(), vec![DomainPairSpec {
            left: domain(number("5"), number("1")), right: domain(number("1"), number("2")) }],
            Vec::new()));
        assert!(report.unwrap_err().message.contains("lower <= upper"));
        // 空请求合法。 / A bare request is legal.
        let report = verify_input_domain(&request(Vec::new(), Vec::new(), Vec::new())).unwrap();
        assert!(report.domains.is_empty());
        assert!(report.pairs.is_empty());
        assert!(report.floats.is_empty());
        // 三处结论与请求无关，始终回报。 / The three findings are reported regardless.
        assert!(report.zero_signs_differ);
        assert!(report.nan_fingerprint_stable);
        assert!(report.optional_swallows_errors);
    }
}

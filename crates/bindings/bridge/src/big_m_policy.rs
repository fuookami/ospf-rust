//! 续挖第 19 单：Big-M 的三条边界与三个指示约束构造器的落地形状
//! （`symbol/functions/big_m.rs`）
//! Continuation, delivery 19: the three Big-M boundaries and the shape the indicator builders emit.
//!
//! 这一层看起来只是"给个 M"，但 `ensure_positive_big_m` 的行为是**两段式**的，而三个构造器的落地形状
//! **并不一致**：
//!
//! 1. **拒绝与夹紧同时存在**：`!finite` 或 `<= 0` 报错，但 **`0 < m < 1` 会被静默抬到 `MIN_BIG_M = 1.0`**。
//!    也就是说 `0.001` 不是被拒，而是**被改写**。这是"看起来校验了、其实只校验了一半"的典型：
//!    调用方传 `0.001` 想表达"几乎不惩罚"，拿到的是 `1.0`，而且没有任何提示。
//! 2. **三个构造器各出 2 条行，但方向与严格性不同**：
//!    - `positive`：两条的指示变量系数都是 **`-M`**；`_ub: <= 0`、`_lb: >= EPSILON - M`。
//!      "严格大于"最终只落到 **`f64::EPSILON`**（≈2.2e-16），不是任何可配的容差。
//!    - `nonnegative`：指示系数 **`-M`**；`_ub: <= 0`、`_lb: >= -M`。
//!    - `negative`：指示系数 **`+M`**（符号反了）；`_lb: >= 0`、`_ub: <= M - EPSILON`。
//! 3. **行的产出顺序也不一致**：`positive`/`nonnegative` 先 `_ub` 后 `_lb`，`negative` 先 `_lb` 后 `_ub`。
//!    对"按下标取第 0 条"的消费方来说，这是真陷阱。
//! 4. 常数项被**留在左端**（`lhs.constant`），没有被挪到 `rhs`。所以 `rhs` 恒为纯数，`lhs` 带常数——
//!    任何"把 lhs 常数搬到右边"的规范化都要自己再做一遍。
//!
//! The layer looks like "just pass an M", but `ensure_positive_big_m` behaves in **two stages** and the three builders
//! **disagree** on the shape they emit. **Refusal and clamping coexist**: `!finite` or `<= 0` is an error, but **`0 < m < 1` is
//! silently raised to `MIN_BIG_M = 1.0`** — `0.001` is not refused, it is **rewritten**, so a caller asking for "almost no
//! penalty" gets `1.0` with no signal. **Each builder emits two rows, with different directions and strictness**: `positive`
//! puts **`-M`** on the indicator in both rows (`_ub: <= 0`, `_lb: >= EPSILON - M`), so "strictly greater" collapses to
//! **`f64::EPSILON`**, not a configurable tolerance; `nonnegative` uses **`-M`** (`_ub: <= 0`, `_lb: >= -M`); `negative` uses
//! **`+M`** — the sign flips — with `_lb: >= 0` and `_ub: <= M - EPSILON`. **The emission order differs too**:
//! `positive`/`nonnegative` emit `_ub` first, `negative` emits `_lb` first, which is a real trap for index-based consumers.
//! And the constant stays **on the left-hand side** (`lhs.constant`) instead of moving to `rhs`, so any canonicalisation that
//! moves it must be done by the caller.

use ospf_rust_core::model::LinearInequality;
use ospf_rust_core::symbol::flatten::{Linear, LinearMonomial};
use ospf_rust_core::symbol::functions::big_m::{
    ensure_positive_big_m, negative_indicator_constraints, nonnegative_indicator_constraints,
    positive_indicator_constraints, DEFAULT_BIG_M, MIN_BIG_M,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 多项式字面量。 / A polynomial literal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BigMPolynomialSpec {
    /// 项（`[列索引, 系数]`）。 / The terms as `[column, coefficient]`.
    #[serde(default)]
    pub terms: Vec<Vec<f64>>,
    /// 常数项（留在左端）。 / The constant, kept on the left-hand side.
    pub constant: f64,
}

/// Big-M 请求。 / The Big-M request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BigMPolicyRequest {
    /// 要过一遍 `ensure_positive_big_m` 的字面量（`"NaN"` / `"inf"` 也接受）。 / Literals to push through the check.
    #[serde(default)]
    pub probes: Vec<String>,
    /// 多项式。 / The polynomial.
    pub polynomial: BigMPolynomialSpec,
    /// 指示变量列。 / The indicator column.
    pub indicator_index: u32,
    /// `M`（会被夹紧到 `>= MIN_BIG_M`）。 / The M, clamped up to `MIN_BIG_M`.
    pub big_m: f64,
    /// 行名前缀。 / The row-name prefix.
    pub name_prefix: String,
}

/// 一次探测。 / One probe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BigMProbeRow {
    /// 输入文本。 / The input text.
    pub input: String,
    /// 是否被接受。 / Whether it was accepted.
    pub accepted: bool,
    /// 接受后的实际值（被夹紧时与输入不同）。 / The value actually used (differs from the input when clamped).
    #[serde(default)]
    pub value: Option<f64>,
    /// 是否被夹紧（输入与结果不同）。 / Whether it was clamped (input differs from result).
    pub clamped: bool,
    /// 错误文本。 / The error text.
    #[serde(default)]
    pub error: Option<String>,
}

/// 一条生成出来的行。 / One emitted row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BigMIndicatorRow {
    /// 构造器名。 / The builder name.
    pub builder: String,
    /// 行名。 / The row name.
    pub name: String,
    /// 关系（`LessEqual` / `GreaterEqual`）。 / The relation.
    pub comparison: String,
    /// 右端常数。 / The right-hand side.
    pub rhs: f64,
    /// 左端常数（**不搬到右边**）。 / The left-hand constant (**not moved to the right**).
    pub lhs_constant: f64,
    /// 指示变量在左端的系数。 / The indicator's coefficient on the left.
    pub indicator_coefficient: f64,
    /// 基础项（`[列索引, 系数]`）。 / The base terms.
    pub terms: Vec<Vec<f64>>,
}

/// Big-M 结论。 / The Big-M verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BigMPolicyReport {
    /// 默认 M。 / The default M.
    pub default_big_m: f64,
    /// 最小 M。 / The minimum M.
    pub min_big_m: f64,
    /// 每个探测的结论。 / Each probe's verdict.
    pub probes: Vec<BigMProbeRow>,
    /// 三个构造器共产出的行（按 `positive` → `nonnegative` → `negative` 排列）。 / Every emitted row.
    pub rows: Vec<BigMIndicatorRow>,
    /// 每个构造器都恰好出 2 条行（**是**）。 / Whether each builder emits exactly two rows (**yes**).
    pub two_rows_each: bool,
    /// 小正数被夹紧而不是被拒（**是**）。 / Whether a small positive is clamped rather than refused (**yes**).
    pub small_positive_is_clamped: bool,
    /// 零被拒（**是**）。 / Whether zero is refused (**yes**).
    pub zero_is_refused: bool,
    /// 严格性就是机器 epsilon（**是**）。 / Whether strictness is just the machine epsilon (**yes**).
    pub strictness_is_machine_epsilon: bool,
    /// `negative` 的指示系数符号与另两个相反（**是**）。 / Whether `negative` flips the indicator sign (**yes**).
    pub negative_flips_indicator_sign: bool,
    /// 产出顺序不一致（**是**）。 / Whether the emission orders differ (**yes**).
    pub orders_differ: bool,
}

/// 探测上限（信任边界）。 / The probe cap (a trust boundary).
const MAX_PROBES: usize = 1_024;

/// 走一遍 Big-M：探测夹紧、跑三个构造器、核对四条一致性。
/// Walk Big-M: probe the clamping, run the three builders, check the four consistency findings.
pub fn verify_big_m_policy(request: &BigMPolicyRequest) -> Result<BigMPolicyReport> {
    if request.probes.len() > MAX_PROBES {
        return Err(BridgeError::new("InvalidInput", &format!(
            "big M failed validation: at most {MAX_PROBES} probes are accepted")));
    }
    let polynomial = build_polynomial(&request.polynomial)?;
    let indicator = usize::try_from(request.indicator_index)
        .map_err(|_| BridgeError::new("InvalidInput", "big M failed validation: indicator overflow"))?;
    if !request.big_m.is_finite() || request.big_m <= 0.0 {
        return Err(BridgeError::new("InvalidInput", &format!(
            "big M failed validation: big M must be finite and positive, got {}", request.big_m)));
    }
    // 构造器会用**夹紧后**的 M，所以这里也只认夹紧后的值；否则严格性判据会拿 0.5 去比 1.0 的行。
    // The builders use the **clamped** M, so this must too; otherwise the strictness predicate compares against 0.5 while
    // the rows hold 1.0.
    let big_m = ensure_positive_big_m(request.big_m).map_err(|error| BridgeError::new("InvalidInput",
        &format!("big M failed validation: {error}")))?;
    if request.name_prefix.trim().is_empty() {
        return Err(BridgeError::new("InvalidInput",
            "big M failed validation: the row-name prefix must not be blank"));
    }
    // 三个构造器用**同一个** M 与多项式，好并排比较。 / The same M and polynomial, to compare side by side.
    let builders: [(&str, IndicatorBuilder); 3] = [
        ("positive", positive_indicator_constraints),
        ("nonnegative", nonnegative_indicator_constraints),
        ("negative", negative_indicator_constraints),
    ];
    let mut rows = Vec::new();
    for (builder, build) in builders {
        let generated = build(&polynomial, indicator, big_m, &request.name_prefix)
            .map_err(|error| BridgeError::new("InvalidInput",
                &format!("big M failed validation: {builder} builder refused: {error}")))?;
        for (row, name) in &generated {
            let mut terms = Vec::with_capacity(row.polynomial.monomials().len());
            let mut indicator_coefficient = 0.0;
            for monomial in row.polynomial.monomials() {
                if monomial.var_index() == indicator {
                    indicator_coefficient = *monomial.coefficient();
                } else {
                    terms.push(vec![monomial.var_index() as f64, *monomial.coefficient()]);
                }
            }
            rows.push(BigMIndicatorRow { builder: builder.to_owned(), name: name.clone(),
                comparison: format!("{:?}", row.relation), rhs: row.rhs,
                lhs_constant: *row.polynomial.constant_term(), indicator_coefficient, terms });
        }
    }
    // 四条实测：每构造器 2 行、顺序不一、符号不一、严格性是机器 epsilon。
    let two_rows_each = ["positive", "nonnegative", "negative"].iter().all(|builder| {
        rows.iter().filter(|row| row.builder == *builder).count() == 2
    });
    let order_of = |builder: &str| rows.iter().filter(|row| row.builder == builder)
        .map(|row| row.name.clone()).collect::<Vec<String>>();
    let orders_differ = order_of("positive") != order_of("negative");
    let sign_of = |builder: &str| rows.iter().find(|row| row.builder == builder)
        .map(|row| row.indicator_coefficient).unwrap_or(0.0);
    let negative_flips_indicator_sign = (sign_of("positive") > 0.0) != (sign_of("negative") > 0.0)
        && sign_of("positive") != 0.0;
    let strictness_is_machine_epsilon = rows.iter().any(|row| {
        (row.rhs - (f64::EPSILON - big_m)).abs() == 0.0
            || (row.rhs - (big_m - f64::EPSILON)).abs() == 0.0
    });
    // 探测：夹紧与拒绝两段。 / Probe the two stages.
    // 这两条是**规则本身**的性质，不是某次请求的性质：探测行只描述单个输入。
    // These two are properties of the **rule**, not of this request; the probe rows describe single inputs.
    let small_positive_is_clamped = ensure_positive_big_m(MIN_BIG_M / 2.0).ok() == Some(MIN_BIG_M);
    let zero_is_refused = ensure_positive_big_m(0.0).is_err();
    let mut probes = Vec::with_capacity(request.probes.len());
    for text in &request.probes {
        let parsed = text.trim().parse::<f64>().map_err(|_| BridgeError::new("InvalidInput",
            &format!("big M failed validation: `{text}` is not a float")))?;
        match ensure_positive_big_m(parsed) {
            Ok(value) => {
                if parsed == 0.0 {
                    return Err(BridgeError::new("InvalidInput",
                        "big M failed validation: a zero big M was accepted"));
                }
                probes.push(BigMProbeRow { input: text.trim().to_owned(), accepted: true,
                    value: Some(value), clamped: value > parsed, error: None });
            }
            Err(error) => {
                probes.push(BigMProbeRow { input: text.trim().to_owned(), accepted: false,
                    value: None, clamped: false, error: Some(error.to_string()) });
            }
        }
    }
    Ok(BigMPolicyReport {
        default_big_m: DEFAULT_BIG_M,
        min_big_m: MIN_BIG_M,
        probes,
        rows,
        two_rows_each,
        small_positive_is_clamped,
        zero_is_refused,
        strictness_is_machine_epsilon,
        negative_flips_indicator_sign,
        orders_differ,
    })
}

/// 三个构造器的签名。 / The three builders' signature.
type IndicatorBuilder = fn(&Linear<f64>, usize, f64, &str)
    -> ospf_rust_core::error::Result<Vec<(LinearInequality<f64>, String)>>;

/// 建一个 `Linear<f64>`。 / Build a `Linear<f64>`.
fn build_polynomial(spec: &BigMPolynomialSpec) -> Result<Linear<f64>> {
    if !spec.constant.is_finite() {
        return Err(BridgeError::new("InvalidInput",
            "big M failed validation: the constant must be finite"));
    }
    let mut monomials = Vec::with_capacity(spec.terms.len());
    for term in &spec.terms {
        if term.len() != 2 {
            return Err(BridgeError::new("InvalidInput",
                "big M failed validation: terms are [column, coefficient] pairs"));
        }
        let column = term[0];
        if !column.is_finite() || column < 0.0 || column.fract() != 0.0 {
            return Err(BridgeError::new("InvalidInput",
                "big M failed validation: the column must be a non-negative integer"));
        }
        if !term[1].is_finite() {
            return Err(BridgeError::new("InvalidInput",
                "big M failed validation: the coefficient must be finite"));
        }
        monomials.push(LinearMonomial::new(term[1], column as usize));
    }
    Ok(Linear::new(monomials, spec.constant))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(probes: &[&str], big_m: f64) -> BigMPolicyRequest {
        BigMPolicyRequest {
            probes: probes.iter().map(|text| (*text).to_owned()).collect(),
            polynomial: BigMPolynomialSpec { terms: vec![vec![0.0, 2.0], vec![1.0, -3.0]],
                constant: 5.0 },
            indicator_index: 2,
            big_m,
            name_prefix: "logic".to_owned(),
        }
    }

    fn row<'a>(report: &'a BigMPolicyReport, builder: &str, name: &str) -> &'a BigMIndicatorRow {
        report.rows.iter().find(|row| row.builder == builder && row.name == name)
            .unwrap_or_else(|| panic!("row {builder}/{name}"))
    }

    #[test]
    fn the_check_refuses_non_positive_but_clamps_small_positives() {
        // `!finite` 与 `<= 0` 报错。 / Non-finite and non-positive are errors.
        for text in ["NaN", "inf", "-inf", "0", "-0", "-1"] {
            assert!(ensure_positive_big_m(text.parse::<f64>().unwrap()).is_err(), "{text}");
        }
        // `0 < m < 1` **被静默抬到 1.0**，不是被拒。 / A small positive is silently raised to 1.0.
        assert_eq!(ensure_positive_big_m(0.5).unwrap(), MIN_BIG_M);
        assert_eq!(ensure_positive_big_m(0.001).unwrap(), MIN_BIG_M);
        assert_eq!(ensure_positive_big_m(1.0).unwrap(), 1.0);
        assert_eq!(ensure_positive_big_m(7.5).unwrap(), 7.5);
        assert_eq!(DEFAULT_BIG_M, 1_000_000.0);
        assert_eq!(MIN_BIG_M, 1.0);
    }

    #[test]
    fn each_builder_emits_two_rows_in_a_different_order() {
        let report = verify_big_m_policy(&request(&[], 10.0)).unwrap();
        assert!(report.two_rows_each);
        assert_eq!(report.rows.len(), 6);
        // `positive` / `nonnegative` 先 `_ub`，`negative` 先 `_lb`。 / The emission orders differ.
        assert_eq!(row(&report, "positive", "logic_ub").comparison, "LessEqual");
        assert_eq!(row(&report, "positive", "logic_lb").comparison, "GreaterEqual");
        assert_eq!(row(&report, "negative", "logic_lb").comparison, "GreaterEqual");
        assert_eq!(row(&report, "negative", "logic_ub").comparison, "LessEqual");
        assert!(report.orders_differ);
        // 三条基础项相同、常数留在左端。 / The base terms agree; the constant stays on the left.
        for row in &report.rows {
            assert_eq!(row.terms, vec![vec![0.0, 2.0], vec![1.0, -3.0]]);
            assert_eq!(row.lhs_constant, 5.0);
        }
    }

    #[test]
    fn indicator_signs_and_strictness_are_measured() {
        let report = verify_big_m_policy(&request(&[], 10.0)).unwrap();
        // `positive` / `nonnegative` 用 `-M`，`negative` 用 `+M`。 / Two use `-M`, `negative` flips it.
        assert_eq!(row(&report, "positive", "logic_ub").indicator_coefficient, -10.0);
        assert_eq!(row(&report, "positive", "logic_lb").indicator_coefficient, -10.0);
        assert_eq!(row(&report, "nonnegative", "logic_ub").indicator_coefficient, -10.0);
        assert_eq!(row(&report, "nonnegative", "logic_lb").indicator_coefficient, -10.0);
        assert_eq!(row(&report, "negative", "logic_lb").indicator_coefficient, 10.0);
        assert_eq!(row(&report, "negative", "logic_ub").indicator_coefficient, 10.0);
        assert!(report.negative_flips_indicator_sign);
        // 严格性就是 `f64::EPSILON`。 / Strictness is just `f64::EPSILON`.
        assert_eq!(row(&report, "positive", "logic_ub").rhs, 0.0);
        assert_eq!(row(&report, "positive", "logic_lb").rhs, f64::EPSILON - 10.0);
        assert_eq!(row(&report, "nonnegative", "logic_ub").rhs, 0.0);
        assert_eq!(row(&report, "nonnegative", "logic_lb").rhs, -10.0);
        assert_eq!(row(&report, "negative", "logic_lb").rhs, 0.0);
        assert_eq!(row(&report, "negative", "logic_ub").rhs, 10.0 - f64::EPSILON);
        assert!(report.strictness_is_machine_epsilon);
        // 小 M 被夹紧后，行里出现的也是夹紧值。 / A clamped M shows up clamped in the rows too.
        let clamped = verify_big_m_policy(&request(&[], 0.5)).unwrap();
        assert!(clamped.small_positive_is_clamped);
        assert_eq!(row(&clamped, "positive", "logic_ub").indicator_coefficient, -MIN_BIG_M);
    }

    #[test]
    fn probes_report_both_stages() {
        let report = verify_big_m_policy(&request(&["0", "0.25", "3", "NaN"], 2.0)).unwrap();
        assert_eq!(report.probes.len(), 4);
        // 零被拒。 / Zero is refused.
        assert!(!report.probes[0].accepted);
        assert!(report.probes[0].error.as_deref().unwrap().contains("finite and positive"));
        assert!(report.zero_is_refused);
        // 小正数被接受但被夹紧。 / A small positive is accepted and clamped.
        assert!(report.probes[1].accepted);
        assert!(report.probes[1].clamped);
        assert_eq!(report.probes[1].value, Some(MIN_BIG_M));
        // 大的原样。 / A large one passes through.
        assert_eq!(report.probes[2].value, Some(3.0));
        assert!(!report.probes[2].clamped);
        // 非有限被拒。 / Non-finite is refused.
        assert!(!report.probes[3].accepted);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_big_m_policy(&request(&["one"], 5.0)).unwrap_err().message
            .contains("is not a float"));
        assert!(verify_big_m_policy(&request(&[], 0.0)).unwrap_err().message
            .contains("finite and positive"));
        assert!(verify_big_m_policy(&request(&[], -2.0)).unwrap_err().message
            .contains("finite and positive"));
        let mut blank = request(&[], 5.0);
        blank.name_prefix = "  ".to_owned();
        assert!(verify_big_m_policy(&blank).unwrap_err().message.contains("prefix"));
        let mut terms = request(&[], 5.0);
        terms.polynomial.terms = vec![vec![0.0]];
        assert!(verify_big_m_policy(&terms).unwrap_err().message.contains("pairs"));
        let mut column = request(&[], 5.0);
        column.polynomial.terms = vec![vec![1.5, 1.0]];
        assert!(verify_big_m_policy(&column).unwrap_err().message.contains("non-negative integer"));
        let mut constant = request(&[], 5.0);
        constant.polynomial.constant = f64::INFINITY;
        assert!(verify_big_m_policy(&constant).unwrap_err().message.contains("constant"));
        let many = request(&vec!["1"; MAX_PROBES + 1], 5.0);
        assert!(verify_big_m_policy(&many).unwrap_err().message.contains("at most"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。这个漏写已经踩过三次，所以每个新模块都留一条。
    /// A guard: DTOs must be camelCase and reject unknown fields. This has been missed three times, so every new module
    /// keeps one.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_big_m_policy(&request(&[], 5.0)).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"defaultBigM\""), "{json}");
        assert!(json.contains("\"negativeFlipsIndicatorSign\""), "{json}");
        assert!(json.contains("\"lhsConstant\""), "{json}");
        assert!(!json.contains("default_big_m"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<BigMPolicyReport>(&copy.to_string()).is_err());
    }
}

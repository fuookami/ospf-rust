//! 续挖第 123 单：**算子能力声明的三条校验只有两条真的在查，且"重复编码"是位置判重而不是排序去重**——
//! `register_*_with_capabilities` 先跑 `OperatorCapabilities::validate`：**`encodings` 不能为空**、**不能与前缀里
//! 任一个相同**（`[C]` 与 `[C, P, C]` 都拒，非相邻重复也拒）；**`stateless` 完全不参与校验**（true/false 都过，
//! 真正的无状态声明由实例自己的 `stateless()` 承担）；能力校验**先于 id 校验**（同一 id 又带坏能力时先报能力那条）；
//! id 与工厂侧的规则是**空或重复 id 被拒**，三个注册面（update/mutation/neighborhood）**各有自己的能力表**，
//! 同一个 id 可以在不同面各注册一次
//! （`operator_registry.rs` 的 `OperatorCapabilities` 与 `register_*_with_capabilities`）
//! Continuation, delivery 123: **the operator capability declaration validates two of its three parts, and "duplicate encoding" is a
//! positional check rather than a sorted one** — `register_*_with_capabilities` first runs `OperatorCapabilities::validate`:
//! **`encodings` must not be empty** and **must not repeat anything already in the prefix** (both `[C]` and `[C, P, C]` are
//! refused, so non-adjacent repeats fail too); **`stateless` takes no part in validation** (true and false both pass, because the
//! real stateless declaration belongs to the instance's own `stateless()`); the capability check **precedes the id check**, so a
//! duplicate id with broken capabilities reports the capability message; the id and factory side refuse **an empty or duplicate
//! id**; and the three registration faces (update/mutation/neighborhood) **each keep their own capability table**, so one id may
//! be registered once per face (`OperatorCapabilities` and `register_*_with_capabilities` in `operator_registry.rs`).
//!
//! 第 122 单量的是超体积，这一单量**算子能力声明的三道门**。八条实测：
//!
//! 1. **空 `encodings` 被拒**（`an_empty_encoding_list_is_refused()`）。
//! 2. **非相邻重复也被拒**（`a_non_adjacent_repeat_is_refused()`）：判重是"与前缀任一相同"。
//! 3. **合法组合被接受并真的注册上**（`a_legal_declaration_registers()`）。
//! 4. **`stateless` 不参与校验**（`the_stateless_flag_is_not_validated()`）。
//! 5. **空 id 被拒**（`an_empty_operator_id_is_refused()`）。
//! 6. **重复 id 被拒**（`a_duplicate_operator_id_is_refused()`）。
//! 7. **能力校验先于 id 校验**（`the_capability_check_precedes_the_id_check()`）。
//! 8. **三个注册面各管各的能力表**（`each_face_keeps_its_own_capabilities()`）：同一 id 可在不同面各注册一次。
//!
//! Delivery 122 measured the hypervolume; this one measures **the operator capability declaration's three gates**.
//! **An empty encoding list is refused** and **a non-adjacent repeat is refused** because the check compares against the whole
//! prefix. **A legal declaration registers**, **the stateless flag is not validated**, **an empty operator id is refused**, and
//! **a duplicate operator id is refused**. **The capability check precedes the id check**, and **each face keeps its own
//! capabilities**.

use ospf_rust_core::solver::heuristic::session::SearchEncoding;
use ospf_rust_core::solver::hyper_heuristic::{
    ComponentSpec, MutationComponent, NeighborhoodComponent, OperatorCapabilities, OperatorRegistry,
    UpdateComponent,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 一次注册尝试。 / One registration attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperatorRegistrationSpec {
    /// 注册面（`update` / `mutation` / `neighborhood`）。 / The registration face.
    pub face: String,
    /// 算子 id。 / The operator id.
    pub id: String,
    /// 声明的编码名（`Continuous` / `Permutation` / `Custom`）。 / The declared encodings.
    #[serde(default)]
    pub encodings: Vec<String>,
    /// 是否声明可作无状态子阶段（默认 false）。 / Whether it declares stateless nesting.
    #[serde(default)]
    pub stateless: Option<bool>,
}

/// 算子能力探针请求。 / The operator capability probe request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperatorCapabilityRequest {
    /// 依次尝试的注册。 / The attempts, in order.
    #[serde(default)]
    pub registrations: Vec<OperatorRegistrationSpec>,
}

/// 一次尝试的读数。 / One attempt's reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperatorRegistrationRow {
    /// 注册面。 / The face.
    pub face: String,
    /// 算子 id。 / The operator id.
    pub id: String,
    /// 是否注册成功。 / Whether it registered.
    pub accepted: bool,
    /// 拒收时的错误文本。 / The refusal text.
    pub error: String,
    /// 该面上累计被接受的注册数（按框架自己的 Ok/Err 累计，注册表没有公开读取口）。
    /// How many registrations that face has accepted so far, tallied from the framework's own verdicts.
    pub accepted_on_face: u32,
    /// 累计被接受的注册总数。 / The total accepted registrations.
    pub total_accepted: u32,
}

/// 算子能力探针结论。 / The operator capability probe verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperatorCapabilityReport {
    /// 各尝试的读数。 / Each attempt's reading.
    pub rows: Vec<OperatorRegistrationRow>,
    /// 八条规则属性的读数。 / The eight rule readings.
    pub an_empty_encoding_list_is_refused: bool,
    pub a_non_adjacent_repeat_is_refused: bool,
    pub a_legal_declaration_registers: bool,
    pub the_stateless_flag_is_not_validated: bool,
    pub an_empty_operator_id_is_refused: bool,
    pub a_duplicate_operator_id_is_refused: bool,
    pub the_capability_check_precedes_the_id_check: bool,
    pub each_face_keeps_its_own_capabilities: bool,
}

/// 编码名。 / The encoding name.
fn encoding_named(name: &str) -> Option<SearchEncoding> {
    match name {
        "Continuous" => Some(SearchEncoding::Continuous),
        "Permutation" => Some(SearchEncoding::Permutation),
        "Custom" => Some(SearchEncoding::Custom),
        _ => None,
    }
}

/// 造一份能力声明。 / Build one capability declaration.
fn capabilities_of(spec: &OperatorRegistrationSpec) -> Result<OperatorCapabilities> {
    let encodings = spec.encodings.iter().map(|name| encoding_named(name.as_str())
        .ok_or_else(|| BridgeError::new("InvalidInput", "operator capability probe failed \
            validation: encoding reads Continuous/Permutation/Custom")))
        .collect::<Result<Vec<_>>>()?;
    Ok(OperatorCapabilities { encodings, stateless: spec.stateless.unwrap_or(false) })
}

/// 走一遍算子能力探针。非法输入是**错误**。 / Walk the probe. An illegal input is an error.
pub fn verify_operator_capability(
    request: &OperatorCapabilityRequest,
) -> Result<OperatorCapabilityReport> {
    if request.registrations.len() > 16 {
        return Err(BridgeError::new("InvalidInput", "operator capability probe failed \
            validation: at most sixteen registrations are accepted"));
    }
    let findings = canonical_findings();
    let mut registry: OperatorRegistry<f64> = OperatorRegistry::new();
    let mut rows = Vec::with_capacity(request.registrations.len());
    // 注册表**没有公开的读取口**（`updates`/`mutations`/… 都是 `pub(crate)`），所以计数只能按框架自己的
    // Ok/Err 判定累计——这仍是实测，只是不从注册表里偷看内部。
    // The registry exposes **no public accessors** (`updates`/`mutations`/… are `pub(crate)`), so the counts tally the
    // framework's own verdicts rather than peeking inside.
    let mut per_face: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    let mut total = 0_u32;
    for spec in &request.registrations {
        let capabilities = capabilities_of(spec)?;
        // 注册失败是**结果**：能力与 id 两处拒收都靠错误文本回读。 
        // A registration failure is a **result**: both the capability and the id refusals come back as text.
        let outcome = match spec.face.as_str() {
            "update" => registry.register_update_with_capabilities(spec.id.clone(),
                capabilities, |_spec: &ComponentSpec| {
                    Err(not_exercised()) as ospf_rust_core::solver::heuristic::result::Result<
                        Box<dyn UpdateComponent<f64>>>
                }),
            "mutation" => registry.register_mutation_with_capabilities(spec.id.clone(),
                capabilities, |_spec: &ComponentSpec| {
                    Err(not_exercised()) as ospf_rust_core::solver::heuristic::result::Result<
                        Box<dyn MutationComponent<f64>>>
                }),
            "neighborhood" => registry.register_neighborhood_with_capabilities(spec.id.clone(),
                capabilities, |_spec: &ComponentSpec| {
                    Err(not_exercised()) as ospf_rust_core::solver::heuristic::result::Result<
                        Box<dyn NeighborhoodComponent<f64>>>
                }),
            _ => return Err(BridgeError::new("InvalidInput", "operator capability probe failed \
                validation: face reads update/mutation/neighborhood")),
        };
        let (accepted, error) = match outcome {
            Ok(()) => (true, String::new()),
            Err(error) => (false, error.to_string()),
        };
        if accepted {
            total += 1;
            let slot = per_face.entry(spec.face.clone()).or_insert(0);
            *slot += 1;
        }
        let accepted_on_face = per_face.get(&spec.face).copied().unwrap_or(0);
        rows.push(OperatorRegistrationRow { face: spec.face.clone(), id: spec.id.clone(),
            accepted, error, accepted_on_face, total_accepted: total });
    }
    Ok(OperatorCapabilityReport {
        rows,
        an_empty_encoding_list_is_refused: findings.an_empty_encoding_list_is_refused,
        a_non_adjacent_repeat_is_refused: findings.a_non_adjacent_repeat_is_refused,
        a_legal_declaration_registers: findings.a_legal_declaration_registers,
        the_stateless_flag_is_not_validated: findings.the_stateless_flag_is_not_validated,
        an_empty_operator_id_is_refused: findings.an_empty_operator_id_is_refused,
        a_duplicate_operator_id_is_refused: findings.a_duplicate_operator_id_is_refused,
        the_capability_check_precedes_the_id_check: findings.the_capability_check_precedes_the_id_check,
        each_face_keeps_its_own_capabilities: findings.each_face_keeps_its_own_capabilities,
    })
}

/// 工厂不会被调用；这里给一个明确的错误。 / The factory is never called; hand back a clear error.
fn not_exercised() -> ospf_rust_core::solver::heuristic::result::HeuristicError {
    ospf_rust_core::solver::heuristic::result::HeuristicError::InvalidArgument(
        "probe factory is never invoked".to_owned())
}

/// 固定探针只跑一次。 / The fixed probes run once only.
static FINDINGS: std::sync::OnceLock<Findings> = std::sync::OnceLock::new();

/// 八条规则属性的内部固定探针。 / The eight rule properties measured by fixed internal probes.
struct Findings {
    an_empty_encoding_list_is_refused: bool,
    a_non_adjacent_repeat_is_refused: bool,
    a_legal_declaration_registers: bool,
    the_stateless_flag_is_not_validated: bool,
    an_empty_operator_id_is_refused: bool,
    a_duplicate_operator_id_is_refused: bool,
    the_capability_check_precedes_the_id_check: bool,
    each_face_keeps_its_own_capabilities: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> &'static Findings {
    FINDINGS.get_or_init(|| {
        let caps = |encodings: Vec<SearchEncoding>, stateless: bool| OperatorCapabilities {
            encodings, stateless };
        // 1) 空编码表被拒。 / An empty encoding list is refused.
        let an_empty_encoding_list_is_refused = {
            let mut registry: OperatorRegistry<f64> = OperatorRegistry::new();
            registry.register_mutation_with_capabilities("m", caps(Vec::new(), false),
                |_spec: &ComponentSpec| Err(not_exercised())).is_err()
        };
        // 2) 非相邻重复也被拒。 / A non-adjacent repeat is refused.
        let a_non_adjacent_repeat_is_refused = {
            let refused = |encodings: Vec<SearchEncoding>| {
                let mut registry: OperatorRegistry<f64> = OperatorRegistry::new();
                registry.register_mutation_with_capabilities("m", caps(encodings, false),
                    |_spec: &ComponentSpec| Err(not_exercised())).is_err()
            };
            // 相邻重复与非相邻重复都拒，而三种各不相同就过。 
            // Adjacent and non-adjacent repeats both fail, while three distinct encodings pass.
            refused(vec![SearchEncoding::Custom, SearchEncoding::Custom])
                && refused(vec![SearchEncoding::Custom, SearchEncoding::Permutation,
                    SearchEncoding::Custom])
                && !refused(vec![SearchEncoding::Continuous, SearchEncoding::Permutation,
                    SearchEncoding::Custom])
        };
        // 3) 合法声明真的注册上。 / A legal declaration really registers.
        let a_legal_declaration_registers = {
            let mut registry: OperatorRegistry<f64> = OperatorRegistry::new();
            let accepted = registry.register_mutation_with_capabilities("m",
                caps(vec![SearchEncoding::Continuous], false),
                |_spec: &ComponentSpec| Err(not_exercised())).is_ok();
            // 注册表没有公开读取口，所以"注册上"只能靠 Ok 判定 + 之后同 id 再注册被拒来证明。 
            // The registry has no public accessor, so "registered" is proven by the Ok verdict plus a duplicate id refusal.
            accepted && registry.register_mutation_with_capabilities("m",
                caps(vec![SearchEncoding::Continuous], false),
                |_spec: &ComponentSpec| Err(not_exercised())).is_err()
        };
        // 4) stateless 不参与校验。 / The stateless flag is not validated.
        let the_stateless_flag_is_not_validated = {
            let mut registry: OperatorRegistry<f64> = OperatorRegistry::new();
            registry.register_mutation_with_capabilities("m",
                caps(vec![SearchEncoding::Continuous], true),
                |_spec: &ComponentSpec| Err(not_exercised())).is_ok()
                && registry.register_neighborhood_with_capabilities("n",
                    caps(vec![SearchEncoding::Continuous], false),
                    |_spec: &ComponentSpec| Err(not_exercised())).is_ok()
        };
        // 5) 空 id 被拒。 / An empty operator id is refused.
        let an_empty_operator_id_is_refused = {
            let mut registry: OperatorRegistry<f64> = OperatorRegistry::new();
            registry.register_mutation_with_capabilities("",
                caps(vec![SearchEncoding::Continuous], false),
                |_spec: &ComponentSpec| Err(not_exercised())).is_err()
        };
        // 6) 重复 id 被拒。 / A duplicate operator id is refused.
        let a_duplicate_operator_id_is_refused = {
            let mut registry: OperatorRegistry<f64> = OperatorRegistry::new();
            let first = registry.register_mutation_with_capabilities("m",
                caps(vec![SearchEncoding::Continuous], false),
                |_spec: &ComponentSpec| Err(not_exercised()));
            let second = registry.register_mutation_with_capabilities("m",
                caps(vec![SearchEncoding::Permutation], false),
                |_spec: &ComponentSpec| Err(not_exercised()));
            first.is_ok() && second.is_err()
        };
        // 7) 能力校验先于 id 校验。 / The capability check precedes the id check.
        let the_capability_check_precedes_the_id_check = {
            let mut registry: OperatorRegistry<f64> = OperatorRegistry::new();
            registry.register_mutation_with_capabilities("m",
                caps(vec![SearchEncoding::Continuous], false),
                |_spec: &ComponentSpec| Err(not_exercised())).expect("the first registration works");
            // 同一 id + 空编码：两条规则都该触发，先报的是能力那条。 
            // The same id with an empty list trips both rules; the capability one reports first.
            let text = registry.register_mutation_with_capabilities("m", caps(Vec::new(), false),
                |_spec: &ComponentSpec| Err(not_exercised())).err()
                .map(|error| error.to_string()).unwrap_or_default();
            text.contains("unique nonempty encodings") && !text.contains("duplicate operator factory")
        };
        // 8) 三个面各管各的能力表。 / Each face keeps its own capability table.
        let each_face_keeps_its_own_capabilities = {
            let mut registry: OperatorRegistry<f64> = OperatorRegistry::new();
            // 同一个 id 在 mutation 与 neighborhood 上各注册一次，两次都成功。 
            // The same id registers once as a mutation and once as a neighborhood, both successfully.
            let mutation = registry.register_mutation_with_capabilities("x",
                caps(vec![SearchEncoding::Continuous], false),
                |_spec: &ComponentSpec| Err(not_exercised()));
            let neighborhood = registry.register_neighborhood_with_capabilities("x",
                caps(vec![SearchEncoding::Permutation], false),
                |_spec: &ComponentSpec| Err(not_exercised()));
            mutation.is_ok() && neighborhood.is_ok()
        };
        Findings {
            an_empty_encoding_list_is_refused,
            a_non_adjacent_repeat_is_refused,
            a_legal_declaration_registers,
            the_stateless_flag_is_not_validated,
            an_empty_operator_id_is_refused,
            a_duplicate_operator_id_is_refused,
            the_capability_check_precedes_the_id_check,
            each_face_keeps_its_own_capabilities,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(request: OperatorCapabilityRequest) -> OperatorCapabilityReport {
        verify_operator_capability(&request).unwrap()
    }

    fn registration(face: &str, id: &str, encodings: &[&str]) -> OperatorRegistrationSpec {
        OperatorRegistrationSpec { face: face.to_owned(), id: id.to_owned(),
            encodings: encodings.iter().map(|name| (*name).to_owned()).collect(),
            stateless: None }
    }

    fn request() -> OperatorCapabilityRequest {
        OperatorCapabilityRequest { registrations: vec![registration("mutation", "m",
            &["Continuous"])] }
    }

    #[test]
    fn the_eight_rules_hold_on_a_plain_run() {
        let report = ask(request());
        assert_eq!(report.rows.len(), 1);
        assert!(report.rows[0].accepted, "{}", report.rows[0].error);
        assert_eq!(report.rows[0].total_accepted, 1);
        assert_eq!(report.rows[0].accepted_on_face, 1);
        assert!(report.an_empty_encoding_list_is_refused);
        assert!(report.a_non_adjacent_repeat_is_refused);
        assert!(report.a_legal_declaration_registers);
        assert!(report.the_stateless_flag_is_not_validated);
        assert!(report.an_empty_operator_id_is_refused);
        assert!(report.a_duplicate_operator_id_is_refused);
        assert!(report.the_capability_check_precedes_the_id_check);
        assert!(report.each_face_keeps_its_own_capabilities);
    }

    #[test]
    fn an_empty_encoding_list_is_refused_with_its_own_message() {
        let report = ask(OperatorCapabilityRequest {
            registrations: vec![registration("mutation", "m", &[])] });
        assert!(!report.rows[0].accepted);
        assert!(report.rows[0].error.contains("unique nonempty encodings"), "{}",
            report.rows[0].error);
        assert_eq!(report.rows[0].total_accepted, 0);
    }

    #[test]
    fn the_duplicate_encoding_check_reads_the_whole_prefix() {
        for encodings in [vec!["Custom", "Custom"],
            vec!["Custom", "Permutation", "Custom"]] {
            let report = ask(OperatorCapabilityRequest {
                registrations: vec![registration("mutation", "m", &encodings)] });
            assert!(!report.rows[0].accepted, "{encodings:?}");
            assert!(report.rows[0].error.contains("unique nonempty encodings"));
        }
        // 三种各不相同就过。 / Three distinct encodings pass.
        let distinct = ask(OperatorCapabilityRequest {
            registrations: vec![registration("mutation", "m",
                &["Continuous", "Permutation", "Custom"])] });
        assert!(distinct.rows[0].accepted, "{}", distinct.rows[0].error);
    }

    #[test]
    fn the_id_rules_and_their_order_show_up() {
        // 空 id 被拒。 / An empty id is refused.
        let blank = ask(OperatorCapabilityRequest {
            registrations: vec![registration("mutation", "", &["Continuous"])] });
        assert!(!blank.rows[0].accepted);
        assert!(blank.rows[0].error.contains("empty or duplicate operator factory ID"),
            "{}", blank.rows[0].error);
        // 同一 id 第二次被拒，且此时报的是 id 那条（能力合法）。 
        // The second registration of one id is refused with the id message because the capabilities are legal.
        let twice = ask(OperatorCapabilityRequest { registrations: vec![
            registration("mutation", "m", &["Continuous"]),
            registration("mutation", "m", &["Permutation"])] });
        assert!(twice.rows[0].accepted);
        assert!(!twice.rows[1].accepted);
        assert!(twice.rows[1].error.contains("empty or duplicate operator factory ID"),
            "{}", twice.rows[1].error);
        // 同一 id + 空编码：先报能力那条。 
        // The same id with an empty list reports the capability message first.
        let ordered = ask(OperatorCapabilityRequest { registrations: vec![
            registration("mutation", "m", &["Continuous"]),
            registration("mutation", "m", &[])] });
        assert!(ordered.rows[1].error.contains("unique nonempty encodings"));
        assert!(!ordered.rows[1].error.contains("duplicate operator factory"));
    }

    #[test]
    fn the_three_faces_are_independent() {
        let report = ask(OperatorCapabilityRequest { registrations: vec![
            registration("mutation", "x", &["Continuous"]),
            registration("neighborhood", "x", &["Permutation"]),
            registration("update", "x", &["Custom"])] });
        assert_eq!(report.rows.len(), 3);
        assert!(report.rows.iter().all(|row| row.accepted));
        assert_eq!(report.rows[2].total_accepted, 3);
        // 但同一个面上再来一次就拒。 / A second registration on the same face is refused.
        let repeated = ask(OperatorCapabilityRequest { registrations: vec![
            registration("mutation", "x", &["Continuous"]),
            registration("mutation", "x", &["Permutation"])] });
        assert!(!repeated.rows[1].accepted);
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(verify_operator_capability(&OperatorCapabilityRequest {
            registrations: vec![registration("nearly", "m", &["Continuous"])] }).unwrap_err()
            .message.contains("face reads update/mutation/neighborhood"));
        assert!(verify_operator_capability(&OperatorCapabilityRequest {
            registrations: vec![registration("mutation", "m", &["Nearly"])] }).unwrap_err()
            .message.contains("encoding reads Continuous/Permutation/Custom"));
        assert!(verify_operator_capability(&OperatorCapabilityRequest {
            registrations: (0..17).map(|index| registration("mutation",
                &format!("m{index}"), &["Continuous"])).collect() }).unwrap_err()
            .message.contains("sixteen registrations"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = ask(request());
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"acceptedOnFace\""), "{json}");
        assert!(json.contains("\"theCapabilityCheckPrecedesTheIdCheck\""), "{json}");
        assert!(!json.contains("accepted_on_face"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<OperatorCapabilityReport>(&copy.to_string()).is_err());
    }
}

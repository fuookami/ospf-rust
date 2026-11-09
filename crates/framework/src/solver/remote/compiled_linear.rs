//! 已编译子模型的无损执行契约，不重新注册 MetaModel 或改写子报告身份。
//! Lossless execution contract for compiled child models, without rebuilding MetaModel or relabeling child reports.

use serde::{Deserialize, Serialize};
use ospf_rust_core::model::ObjectiveCategory;
use ospf_rust_core::model::intermediate::{BasicLinearTriadModel, LinearTriadModel, SparseVector};
use ospf_rust_core::solver::{ModelFingerprint, linear_model_fingerprint, linear_model_mapping, sha256_fingerprint};
use ospf_rust_core::token::{AnyVariable, Token, TokenVariableData};
use ospf_rust_core::variable::{VariableType, new_standalone_id};
use super::{RemoteSolverError, RemoteSolverResult, SerializedConstraintSign, SerializedLinearModel, SerializedObjectiveCategory, serialize_linear_model};

const MAX_BYTES: usize = 8 * 1024 * 1024;

/// 实际编译模型的独立封装；只在 Rust 解析 modelJson，局部 arena 句柄不跨进程。
/// Independent actual-compiled-model artifact; only Rust parses modelJson, and local arena handles do not cross processes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompiledLinearModelArtifact {
    /// 独立执行 codec 版本，不改变既有远程模型 schema。 / Independent execution-codec version, not a change to the existing remote-model schema.
    pub schema_version: String,
    /// 复用既有稀疏模型 DTO，并补全类型及元数据的 canonical 文本。 / Canonical text reusing existing sparse model DTOs with complete types and metadata.
    pub model_json: String,
    /// 实际子模型 core 指纹，必须与原报告相同。 / Actual child core fingerprint, which must match the original report.
    pub model_fingerprint: ModelFingerprint,
    /// 覆盖所有载荷字节，包括 core 指纹未覆盖的 solver 类型。 / Digest of all payload bytes, including solver types not covered by the core fingerprint.
    pub content_fingerprint: ModelFingerprint,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CompiledLinearPayload {
    model: SerializedLinearModel,
    token_types: Vec<VariableType>,
    solver_types: Vec<VariableType>,
    constraint_names: Vec<String>,
    constraint_group_ids: Vec<Option<u64>>,
    constraint_lazy_flags: Vec<bool>,
    constraint_priorities: Vec<u32>,
    constraint_args: Vec<Option<String>>,
    constraint_source_symbol_ids: Vec<Option<u64>>,
}

fn invalid(error: impl ToString) -> RemoteSolverError { RemoteSolverError::invalid_argument(error.to_string()) }

impl CompiledLinearPayload {
    fn from_model(model: &LinearTriadModel) -> Self {
        Self { model: serialize_linear_model(model), token_types: model.basic.variables.iter().map(Token::var_type).collect(),
            solver_types: model.basic.var_types.clone(), constraint_names: model.basic.constraint_names.clone(),
            constraint_group_ids: model.basic.constraint_group_ids.clone(), constraint_lazy_flags: model.basic.constraint_lazy_flags.clone(),
            constraint_priorities: model.basic.constraint_priorities.clone(), constraint_args: model.basic.constraint_args.clone(),
            constraint_source_symbol_ids: model.basic.constraint_source_symbol_ids.clone() }
    }

    fn compile(self) -> RemoteSolverResult<LinearTriadModel> {
        self.model.validate_contract()?;
        let input = self.model;
        let count = input.variables.len();
        let rows = input.constraints.len();
        if input.schema_version != "1.0" || self.token_types.len() != count || self.solver_types.len() != count
            || self.constraint_names.len() > rows || self.constraint_group_ids.len() > rows || self.constraint_lazy_flags.len() > rows
            || self.constraint_priorities.len() > rows || self.constraint_args.len() > rows || self.constraint_source_symbol_ids.len() > rows
            || input.objective.constant != 0.0 { return Err(invalid("invalid compiled linear model shape/schema/constant")); }
        let mut basic = BasicLinearTriadModel::new(&input.name);
        for (index, variable) in input.variables.iter().enumerate() {
            if variable.index != index { return Err(invalid("compiled linear variable order is not contiguous")); }
            // 新 ID 只供此解码实例的索引；报告身份由原名称、列号、类型及行来源重建。
            // Fresh local IDs serve this decoded instance only; report identity uses original names, columns, types and row origins.
            let token = Token::new(AnyVariable::new(TokenVariableData {
                id: new_standalone_id(), index, name: variable.name.clone(), display_name: None,
                var_type: self.token_types[index], lower_bound: Some(variable.lower_bound), upper_bound: Some(variable.upper_bound),
            }), index);
            basic.add_variable_with_bounds(token, variable.lower_bound, variable.upper_bound, self.solver_types[index]);
        }
        for (index, row) in input.constraints.iter().enumerate() {
            if row.sign != SerializedConstraintSign::LessEqual || !row.rhs.is_finite() { return Err(invalid("compiled rows must be finite standard-form inequalities")); }
            let mut entries = SparseVector::new();
            for cell in &row.cells {
                if cell.row_index != index || cell.col_index >= count || !cell.coefficient.is_finite() { return Err(invalid("invalid compiled sparse row")); }
                entries.add(cell.col_index, cell.coefficient);
            }
            basic.A.add_row(entries);
            basic.b.push(row.rhs);
        }
        basic.constraint_names = self.constraint_names;
        basic.constraint_group_ids = self.constraint_group_ids;
        basic.constraint_lazy_flags = self.constraint_lazy_flags;
        basic.constraint_priorities = self.constraint_priorities;
        basic.constraint_args = self.constraint_args;
        basic.constraint_source_symbol_ids = self.constraint_source_symbol_ids;
        let mut model = LinearTriadModel::from_basic(basic);
        model.c = vec![0.0; count];
        for cell in &input.objective.cells {
            if cell.col_index >= count || !cell.coefficient.is_finite() { return Err(invalid("invalid compiled objective cell")); }
            model.c[cell.col_index] += cell.coefficient;
        }
        model.objective_category = match input.objective.category {
            SerializedObjectiveCategory::Minimize => ObjectiveCategory::Minimum,
            SerializedObjectiveCategory::Maximize => ObjectiveCategory::Maximum,
            _ => return Err(invalid("unsupported compiled objective category")),
        };
        let mapping = linear_model_mapping(&model).map_err(invalid)?;
        if input.variables.iter().zip(&mapping.variables_by_column).any(|(value, id)| value.stable_id != id.0)
            || input.constraints.iter().zip(&mapping.constraints_by_row).any(|(value, id)| value.stable_id != *id)
            || input.objective.stable_id != mapping.objective_id { return Err(invalid("compiled linear model stable mapping mismatch")); }
        Ok(model)
    }
}

/// 编码实际模型；不经 MetaModel 注册，不丢弃类型、空缺元数据或局部行身份。
/// Encode an actual model without MetaModel registration or loss of types, absent metadata or local row identity.
pub fn encode_compiled_linear_model(model: &LinearTriadModel) -> RemoteSolverResult<CompiledLinearModelArtifact> {
    let model_fingerprint = linear_model_fingerprint(model).map_err(invalid)?;
    let model_json = serde_json::to_string(&CompiledLinearPayload::from_model(model)).map_err(invalid)?;
    if model_json.len() > MAX_BYTES { return Err(invalid("compiled linear model exceeds 8 MiB")); }
    let artifact = CompiledLinearModelArtifact { schema_version: "1.0".into(),
        content_fingerprint: sha256_fingerprint("ospf.compiled-linear-model.v1", model_json.as_bytes()), model_fingerprint, model_json };
    decode_compiled_linear_model(&artifact)?;
    Ok(artifact)
}

/// 校验版本、完整载荷摘要、canonical 形式、原始 core 指纹及稳定映射后还原模型。
/// Restore a model after verifying version, full-payload digest, canonical form, original core fingerprint and stable mapping.
pub fn decode_compiled_linear_model(artifact: &CompiledLinearModelArtifact) -> RemoteSolverResult<LinearTriadModel> {
    if artifact.schema_version != "1.0" { return Err(invalid("unsupported compiled linear model schema")); }
    if artifact.model_json.len() > MAX_BYTES { return Err(invalid("compiled linear model exceeds 8 MiB")); }
    if sha256_fingerprint("ospf.compiled-linear-model.v1", artifact.model_json.as_bytes()) != artifact.content_fingerprint {
        return Err(invalid("compiled linear model content fingerprint mismatch"));
    }
    let input: CompiledLinearPayload = serde_json::from_str(&artifact.model_json).map_err(invalid)?;
    let model = input.compile()?;
    if linear_model_fingerprint(&model).map_err(invalid)? != artifact.model_fingerprint {
        return Err(invalid("compiled linear model core fingerprint mismatch"));
    }
    if serde_json::to_string(&CompiledLinearPayload::from_model(&model)).map_err(invalid)? != artifact.model_json {
        return Err(invalid("compiled linear model payload is not canonical or loses execution data"));
    }
    Ok(model)
}

#[cfg(test)]
mod tests;

//! 独立拥有、显式释放的原分区生命周期对象，不执行原生 solver。
//! Independently owned, explicitly disposable original partition lifecycle objects, without native solver execution.

use serde::Serialize;
use wasm_bindgen::prelude::*;
use ospf_rust_bridge::{BppLoadingPartitionBatch, BppLoadingPartitionContext, BppLoadingPartitionMaster, BppLoadingPartitionPricing, BridgeError};
use super::{DiscreteLoadingResult, MAX_BYTES, parse};
use crate::{bridge_error, error};

fn json<T: Serialize + ?Sized>(value: &T) -> Result<String, JsValue> {
    let text = serde_json::to_string(value).map_err(error)?;
    if text.len() > MAX_BYTES { return Err(bridge_error(BridgeError::new("PayloadTooLarge", "BPP partition DTO exceeds 8 MiB"))); }
    Ok(text)
}

/// 原分区 context 的独立会话，不借用 JS 离散 context。 / Independent original partition context session, without borrowing a JS discrete context.
#[wasm_bindgen]
pub struct PartitionLoadingContext { value: BppLoadingPartitionContext }

#[wasm_bindgen]
impl PartitionLoadingContext {
    /// 从无损源及生成的分区配置独立注册。 / Independently register from a lossless source and generated partition configuration.
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str, options: &str) -> Result<PartitionLoadingContext, JsValue> {
        Ok(Self { value: BppLoadingPartitionContext::new(&parse(source, MAX_BYTES * 2 + 1024)?, parse(options, MAX_BYTES)?).map_err(bridge_error)? })
    }

    /// 独立源封装。 / Independent source artifact.
    #[wasm_bindgen(js_name = toArtifactJson)]
    pub fn to_artifact_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.artifact()).map_err(error) }

    /// 显式配置快照。 / Explicit configuration snapshot.
    #[wasm_bindgen(js_name = optionsJson)]
    pub fn options_json(&self) -> Result<String, JsValue> { json(&self.value.options()) }

    /// 不含裸 token 的领域决策目录。 / Domain decision catalog without raw tokens.
    #[wasm_bindgen(js_name = decisionsJson)]
    pub fn decisions_json(&self) -> Result<String, JsValue> { json(self.value.decisions()) }

    /// 含精确件数文本及单位的完整列元数据。 / Complete column metadata with exact count text and units.
    #[wasm_bindgen(js_name = columnsJson)]
    pub fn columns_json(&self) -> Result<String, JsValue> { json(&self.value.columns()) }

    /// 全批成功后才修改列池。 / Mutate the pool only after whole-batch success.
    #[wasm_bindgen(js_name = addColumns)]
    pub fn add_columns(&mut self, columns: &str) -> Result<(), JsValue> { self.value.add_columns(parse(columns, MAX_BYTES)?).map_err(bridge_error) }

    /// 原子删除并使旧注册失效。 / Atomically remove columns and invalidate old registrations.
    #[wasm_bindgen(js_name = removeColumns)]
    pub fn remove_columns(&mut self, ids: &str) -> Result<(), JsValue> { self.value.remove_columns(parse(ids, MAX_BYTES)?).map_err(bridge_error) }

    /// 创建独立主问题对象，不执行求解。 / Create an independent master object, without solving.
    pub fn master(&mut self, options: &str) -> Result<PartitionLoadingMaster, JsValue> {
        Ok(PartitionLoadingMaster { value: self.value.master(parse(options, MAX_BYTES)?).map_err(bridge_error)? })
    }

    /// 完整 LP 证书门禁，原 u64 元数据不经过 usize。 / Full LP certificate gate without routing original u64 metadata through usize.
    #[wasm_bindgen(js_name = acceptLpReport)]
    pub fn accept_lp_report(&mut self, master: &PartitionLoadingMaster, text: &str) -> Result<String, JsValue> { json(&self.value.accept_lp_report(&master.value, text).map_err(bridge_error)?) }

    /// 无对偶种子请求。 / Seed request without duals.
    #[wasm_bindgen(js_name = seedPricing)]
    pub fn seed_pricing(&self, partition: &str, phase_one: bool) -> Result<PartitionLoadingPricing, JsValue> {
        Ok(PartitionLoadingPricing { value: self.value.seed_pricing(partition, phase_one).map_err(bridge_error)? })
    }

    /// 本会话当前主问题的已验证定价请求。 / Verified pricing request for this session's current master.
    pub fn pricing(&self, master: &PartitionLoadingMaster, partition: &str, phase_one: bool) -> Result<PartitionLoadingPricing, JsValue> {
        Ok(PartitionLoadingPricing { value: self.value.pricing(&master.value, partition, phase_one).map_err(bridge_error)? })
    }

    /// 一次消费同轮批次，不复用失效对偶。 / Consume a round batch once, without reusing invalidated duals.
    #[wasm_bindgen(js_name = addPricedBatch)]
    pub fn add_priced_batch(&mut self, master: &PartitionLoadingMaster, batch: &PartitionLoadingBatch) -> Result<(), JsValue> { self.value.add_priced_batch(&master.value, &batch.value).map_err(bridge_error) }

    /// 返回独立完整源向量。 / Return an independent complete source vector.
    #[wasm_bindgen(js_name = sourceValues)]
    pub fn source_values(&self, master: &PartitionLoadingMaster, values: &[f64]) -> Result<Vec<f64>, JsValue> { self.value.source_values(&master.value, values).map_err(bridge_error) }

    /// 完整模型和领域双重验证后的独立方案。 / Independent plan after complete model and domain validation.
    #[wasm_bindgen(js_name = extractSolution)]
    pub fn extract_solution(&self, master: &PartitionLoadingMaster, values: &[f64]) -> Result<DiscreteLoadingResult, JsValue> {
        Ok(DiscreteLoadingResult { value: self.value.extract_solution(&master.value, values).map_err(bridge_error)? })
    }

    /// 物化白名单资源补全，并使旧主问题失效。 / Materialize whitelisted resource completion and invalidate the old master.
    #[wasm_bindgen(js_name = materializeCompletedResources)]
    pub fn materialize_completed_resources(&mut self, master: &PartitionLoadingMaster, values: &[f64]) -> Result<(), JsValue> { self.value.materialize_completed_resources(&master.value, values).map_err(bridge_error) }
}

/// 独立注册快照及只读索引。 / Independent registration snapshot and read-only indices.
#[wasm_bindgen]
pub struct PartitionLoadingMaster { value: BppLoadingPartitionMaster }

#[wasm_bindgen]
impl PartitionLoadingMaster {
    /// 本次实际编译模型。 / Actual model of this compilation.
    #[wasm_bindgen(js_name = toModelJson)]
    pub fn to_model_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.artifact()).map_err(error) }
    /// 不包含在 backend c*x 中的独立常数。 / Separate constant excluded from backend c*x.
    #[wasm_bindgen(js_name = objectiveConstant)]
    pub fn objective_constant(&self) -> f64 { self.value.objective_constant() }
    /// 列到主问题向量索引。 / Columns to master-vector indices.
    #[wasm_bindgen(js_name = columnIndicesJson)]
    pub fn column_indices_json(&self) -> Result<String, JsValue> { json(self.value.column_indices()) }
    /// 保留的全局／补全资源索引。 / Retained global/completion-resource indices.
    #[wasm_bindgen(js_name = globalBindingsJson)]
    pub fn global_bindings_json(&self) -> Result<String, JsValue> { json(self.value.global_bindings()) }
}

/// 只读定价快照，可在 context 释放后读取，但不能用于新会话加列。 / Read-only pricing snapshot, readable after context disposal but unusable for adding columns to new sessions.
#[wasm_bindgen]
pub struct PartitionLoadingPricing { value: BppLoadingPartitionPricing }

#[wasm_bindgen]
impl PartitionLoadingPricing {
    /// 包含原布局辅助变量的实际局部模型。 / Actual local model retaining original layout auxiliaries.
    #[wasm_bindgen(js_name = toModelJson)]
    pub fn to_model_json(&self) -> Result<String, JsValue> { serde_json::to_string(&self.value.artifact()).map_err(error) }
    /// 该快照的凸性价格常数。 / Convexity-price constant of this snapshot.
    #[wasm_bindgen(js_name = objectiveConstant)]
    pub fn objective_constant(&self) -> f64 { self.value.objective_constant() }
    /// 分区身份。 / Partition identity.
    #[wasm_bindgen(js_name = partitionId)]
    pub fn partition_id(&self) -> String { self.value.partition_id().into() }
    /// 仅计算本快照检验数，不断言会话仍有效。 / Evaluate reduced cost for this snapshot only, without asserting current session validity.
    #[wasm_bindgen(js_name = reducedCost)]
    pub fn reduced_cost(&self, values: &str) -> Result<f64, JsValue> { self.value.reduced_cost(&parse(values, MAX_BYTES)?).map_err(bridge_error) }
    /// 完整原报告及局部方案门禁，原文无损保留。 / Full original-report and local-plan gate, retaining lossless original text.
    #[wasm_bindgen(js_name = readReportJson)]
    pub fn read_report_json(&self, text: &str) -> Result<String, JsValue> { serde_json::to_string(&self.value.read_report(text).map_err(bridge_error)?).map_err(error) }
}

/// 同一 LP epoch 的完整候选批次。 / Complete candidate batch for one LP epoch.
#[wasm_bindgen]
pub struct PartitionLoadingBatch { value: BppLoadingPartitionBatch }

#[wasm_bindgen]
impl PartitionLoadingBatch {
    /// 创建空批次，不借用请求对象。 / Create an empty batch, without borrowing request objects.
    #[wasm_bindgen(constructor)]
    pub fn new() -> PartitionLoadingBatch { Self { value: Default::default() } }
    /// 复制请求和候选，不修改源池。 / Copy the request and candidates without changing the source pool.
    pub fn add(&mut self, pricing: &PartitionLoadingPricing, columns: &str) -> Result<(), JsValue> { self.value.push(&pricing.value, parse(columns, MAX_BYTES)?).map_err(bridge_error) }
}

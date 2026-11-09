/// 列生成失败阶段 / Column-generation failure stage
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnGenerationFailureStage {
    /// 初始列 / Initial columns
    InitialColumns,
    /// RMP 求解 / Restricted master problem
    RestrictedMasterProblem,
    /// 最终 MILP 求解 / Final MILP
    FinalMilp,
    /// 候选过滤 / Candidate filter
    CandidateFilter,
    /// 解分析 / Solution analysis
    SolutionAnalysis,
}

/// 执行器失败 / Executor failure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnGenerationExecutionError {
    /// 失败阶段 / Failure stage
    pub stage: ColumnGenerationFailureStage,
    /// 原始错误 / Original error
    pub message: String,
}

fn rmp_execution_result(execution: ColumnGenerationRmpExecution) -> Result<ColumnGenerationRmpExecution, ColumnGenerationExecutionError> {
    if matches!(execution.info.get("status").map(String::as_str), Some("registration_failed" | "solve_failed")) {
        return Err(ColumnGenerationExecutionError {
            stage: ColumnGenerationFailureStage::RestrictedMasterProblem,
            message: execution.info.get("error").cloned().unwrap_or_else(|| "RMP execution failed".to_string()),
        });
    }
    Ok(execution)
}

fn final_execution_result(execution: ColumnGenerationFinalExecution) -> Result<ColumnGenerationFinalExecution, ColumnGenerationExecutionError> {
    if matches!(execution.info.get("status").map(String::as_str), Some("registration_failed" | "solve_failed" | "physical_validation_failed")) {
        return Err(ColumnGenerationExecutionError {
            stage: ColumnGenerationFailureStage::FinalMilp,
            message: execution.info.get("error").cloned().unwrap_or_else(|| "final MILP execution failed".to_string()),
        });
    }
    Ok(execution)
}

/// RMP 模型扩展 / RMP model extension
pub trait ColumnGenerationRmpModelExtension: Debug + Send + Sync {
    /// 注册额外模型行 / Register additional model rows
    fn register(
        &self,
        state: &ColumnGenerationApplicationState,
        model: &mut MetaModel<f64>,
    ) -> Result<(), String>;

    /// 提取类型化附加对偶值 / Extract typed additional dual values
    fn additional_shadow_prices(
        &self,
        _state: &ColumnGenerationApplicationState,
        _model: &MetaModel<f64>,
        _solve: &MetaModelExecutorSolveResult,
    ) -> HashMap<String, f64> {
        HashMap::new()
    }
}

/// final 模型扩展 / Final model extension
pub trait ColumnGenerationFinalModelExtension: Debug + Send + Sync {
    /// 注册额外最终模型内容 / Register additional final-model content
    fn register(
        &self,
        state: &ColumnGenerationApplicationState,
        model: &mut MetaModel<f64>,
    ) -> Result<(), String>;
}

/// RMP 执行器 / RMP executor
pub trait ColumnGenerationRmpExecutor: Debug + Send + Sync {
    /// 执行 RMP / Execute RMP
    fn execute(&self, state: &ColumnGenerationApplicationState) -> ColumnGenerationRmpExecution;

    /// 以 Result 传播失败 / Execute with typed failure propagation
    fn execute_result(
        &self,
        state: &ColumnGenerationApplicationState,
    ) -> Result<ColumnGenerationRmpExecution, ColumnGenerationExecutionError> {
        rmp_execution_result(self.execute(state))
    }

    /// 执行与物品目录同序的数量需求；旧执行器默认拒绝非单位需求。
    /// Execute item quantities in catalog order; legacy executors reject non-unit demand by default.
    fn execute_result_with_item_amounts(
        &self,
        state: &ColumnGenerationApplicationState,
        amounts: &[u64],
    ) -> Result<ColumnGenerationRmpExecution, ColumnGenerationExecutionError> {
        validate_item_amounts(&state.items, amounts).map_err(|message| ColumnGenerationExecutionError {
            stage: ColumnGenerationFailureStage::RestrictedMasterProblem, message,
        })?;
        if amounts.iter().any(|amount| *amount != 1) {
            return Err(ColumnGenerationExecutionError {
                stage: ColumnGenerationFailureStage::RestrictedMasterProblem,
                message: "RMP executor does not support grouped item demand".into(),
            });
        }
        self.execute_result(state)
    }
}

/// 最终 MILP 执行器 / Final MILP executor
pub trait ColumnGenerationFinalExecutor: Debug + Send + Sync {
    /// 执行 final MILP / Execute final MILP
    fn execute(&self, state: &ColumnGenerationApplicationState) -> ColumnGenerationFinalExecution;

    /// 以 Result 传播失败 / Execute with typed failure propagation
    fn execute_result(
        &self,
        state: &ColumnGenerationApplicationState,
    ) -> Result<ColumnGenerationFinalExecution, ColumnGenerationExecutionError> {
        final_execution_result(self.execute(state))
    }

    /// 执行与物品目录同序的数量需求；旧执行器默认拒绝非单位需求。
    /// Execute item quantities in catalog order; legacy executors reject non-unit demand by default.
    fn execute_result_with_item_amounts(
        &self,
        state: &ColumnGenerationApplicationState,
        amounts: &[u64],
    ) -> Result<ColumnGenerationFinalExecution, ColumnGenerationExecutionError> {
        validate_item_amounts(&state.items, amounts).map_err(|message| ColumnGenerationExecutionError {
            stage: ColumnGenerationFailureStage::FinalMilp, message,
        })?;
        if amounts.iter().any(|amount| *amount != 1) {
            return Err(ColumnGenerationExecutionError {
                stage: ColumnGenerationFailureStage::FinalMilp,
                message: "final executor does not support grouped item demand".into(),
            });
        }
        self.execute_result(state)
    }
}

// MetaModel 执行诊断已上移到 `ospf-rust-framework`，见 `executor/backend.rs` 的兼容 re-export。
// MetaModel execution diagnostics moved to `ospf-rust-framework`; see `executor/backend.rs` for the
// compatibility re-export.


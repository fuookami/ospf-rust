/// 一轮分组求解的执行配置。 / Execution settings for one grouped-demand round.
#[derive(Clone, Default)]
pub struct Bpp3dOneRoundOptions {
    /// 子求解共享的预算、取消和进度。 / Budget, cancellation and progress shared by child solves.
    pub linear: ospf_rust_framework::solver::LinearExecutionOptions,
    /// 严格物理复验配置。 / Strict physical replay settings.
    pub validation: FinalPackingValidation,
}

/// 一次 RMP 的实际输入、输出和已接受的原报告。 / Actual inputs, output and accepted original report for one RMP.
#[derive(Debug, Clone)]
pub struct Bpp3dRmpStageEvidence {
    /// 求解时使用的列池、trace 和物品。 / Column pool, traces and items used for this solve.
    pub state: ColumnGenerationApplicationState,
    /// 按物品目录顺序的需求；None 表示未显式传入数量。 / Demand in item-catalog order; None means no explicit amounts.
    pub amounts: Option<Vec<u64>>,
    /// 原领域执行结果，包含注册或求解失败。 / Original domain execution, including registration or solve failures.
    pub execution: ColumnGenerationRmpExecution,
    /// 已通过后端门禁的实际模型和原报告；提交前或契约失败时为空。 / Accepted actual model and report, absent on pre-submission or contract failures.
    pub report: Option<ospf_rust_framework::solver::LinearStageReport<MetaModelStage>>,
}

/// final 的实际输入、分配和原报告，物理失败不丢弃证据。 / Actual final inputs, selection and report; physical failures retain evidence.
#[derive(Debug, Clone)]
pub struct Bpp3dFinalStageEvidence {
    /// final 使用的完整列池及物理 trace。 / Complete final column pool and physical traces.
    pub state: ColumnGenerationApplicationState,
    /// 按物品目录顺序的需求；None 表示未显式传入数量。 / Demand in item-catalog order; None means no explicit amounts.
    pub amounts: Option<Vec<u64>>,
    /// 原领域结果和实际提取的箱／层分配。 / Original domain result and actually extracted bin/layer selection.
    pub execution: LayerAssignmentFinalExecution,
    /// 已通过后端门禁的实际模型和原报告；提交前或契约失败时为空。 / Accepted actual model and report, absent on pre-submission or contract failures.
    pub report: Option<ospf_rust_framework::solver::LinearStageReport<MetaModelStage>>,
}

/// 单轮实际流程及其子阶段证据；失败时已执行的阶段仍保留。 / Actual one-round flow and stage evidence; completed stages survive failures.
#[derive(Debug, Clone)]
pub struct Bpp3dOneRoundEvidence {
    /// 原 application 成功结果或失败信息。 / Original application success or failure.
    pub flow: Result<ColumnGenerationApplicationFlowResult, Vec<String>>,
    /// 按实际调用顺序保存的 RMP；最多两次。 / RMPs in actual invocation order, at most two.
    pub rmp: Vec<Bpp3dRmpStageEvidence>,
    /// 已到达 final 时的证据。 / Final evidence when the flow reaches it.
    pub final_stage: Option<Bpp3dFinalStageEvidence>,
}

#[derive(Debug)]
struct EvidenceRmpExecutor {
    backend: ReportMetaModelBackend,
    records: std::sync::Mutex<Vec<Bpp3dRmpStageEvidence>>,
}

impl EvidenceRmpExecutor {
    fn run(&self, state: &ColumnGenerationApplicationState, amounts: Option<&[u64]>) -> ColumnGenerationRmpExecution {
        let previous = self.backend.reports().len();
        let execution = MetaModelRmpExecutor::default().execute_with_backend_and_amounts(state, &self.backend, None, amounts);
        let report = self.backend.reports().get(previous).cloned();
        self.records.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(Bpp3dRmpStageEvidence {
            state: state.clone(), amounts: amounts.map(<[u64]>::to_vec), execution: execution.clone(), report,
        });
        execution
    }
}

impl ColumnGenerationRmpExecutor for EvidenceRmpExecutor {
    fn execute(&self, state: &ColumnGenerationApplicationState) -> ColumnGenerationRmpExecution {
        self.run(state, None)
    }

    fn execute_result_with_item_amounts(
        &self,
        state: &ColumnGenerationApplicationState,
        amounts: &[u64],
    ) -> Result<ColumnGenerationRmpExecution, ColumnGenerationExecutionError> {
        rmp_execution_result(self.run(state, Some(amounts)))
    }
}

#[derive(Debug)]
struct EvidenceFinalExecutor {
    executor: TraceValidatedMetaModelFinalExecutor,
    record: std::sync::Mutex<Option<Bpp3dFinalStageEvidence>>,
}

impl EvidenceFinalExecutor {
    fn run(&self, state: &ColumnGenerationApplicationState, amounts: Option<&[u64]>) -> ColumnGenerationFinalExecution {
        let previous = self.executor.backend.reports().len();
        let execution = match amounts {
            Some(amounts) => self.executor.execute_with_selection_and_item_amounts(state, amounts, None),
            None => self.executor.execute_with_selection(state, None),
        };
        let report = self.executor.backend.reports().get(previous).cloned();
        let result = execution.execution.clone();
        *self.record.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Bpp3dFinalStageEvidence {
            state: state.clone(), amounts: amounts.map(<[u64]>::to_vec), execution, report,
        });
        result
    }
}

impl ColumnGenerationFinalExecutor for EvidenceFinalExecutor {
    fn execute(&self, state: &ColumnGenerationApplicationState) -> ColumnGenerationFinalExecution {
        self.run(state, None)
    }

    fn execute_result_with_item_amounts(
        &self,
        state: &ColumnGenerationApplicationState,
        amounts: &[u64],
    ) -> Result<ColumnGenerationFinalExecution, ColumnGenerationExecutionError> {
        final_execution_result(self.run(state, Some(amounts)))
    }
}

impl<G> ColumnGenerationApplicationService<G>
where
    G: PackingGeometryContract<f64, Meter>,
{
    /// 使用新建的报告后端执行真实单轮流程，保留每次输入、原报告及严格 final 分配。
    /// Run the actual one-round flow with a fresh report backend, retaining each input, original report and strict final selection.
    /// 不重新求解，也不将受限模型的证明提升为源问题证明；流程失败仍返回已取得的证据。
    /// Never re-solve or promote restricted-model proofs to source-wide proofs; flow failures still return acquired evidence.
    pub fn run_csv_materialized_one_generation_round_with_evidence(
        &self,
        request: CsvMaterializedApplicationRequest,
        initial_traces: Vec<LayerTracePair>,
        layer_generation: LayerGenerationContext<f64, Meter>,
        solver: std::sync::Arc<dyn ospf_rust_core::solver::LinearSolver>,
        options: Bpp3dOneRoundOptions,
    ) -> ospf_rust_core::Result<Bpp3dOneRoundEvidence> {
        if !(1..=100_000).contains(&options.validation.max_placements) {
            return Err(ospf_rust_core::error::SolverError::InvalidInput("invalid strict replay placement limit".into()).into());
        }
        let backend = ReportMetaModelBackend::new(solver, options.linear)?;
        let rmp = EvidenceRmpExecutor { backend: backend.clone(), records: std::sync::Mutex::new(Vec::new()) };
        let mut strict = TraceValidatedMetaModelFinalExecutor::new(Default::default(), backend);
        strict.validation = options.validation;
        let final_executor = EvidenceFinalExecutor { executor: strict, record: std::sync::Mutex::new(None) };
        let flow = self.run_csv_materialized_one_generation_round_with_traced_initial_layers(
            request, initial_traces, layer_generation, &rmp, &final_executor,
        );
        Ok(Bpp3dOneRoundEvidence {
            flow,
            rmp: rmp.records.into_inner().unwrap_or_else(|poisoned| poisoned.into_inner()),
            final_stage: final_executor.record.into_inner().unwrap_or_else(|poisoned| poisoned.into_inner()),
        })
    }
}

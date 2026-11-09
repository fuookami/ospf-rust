//! 实际已编译子模型执行，保留原 token、solver 松弛类型及报告身份。
//! Execute actual compiled child models, retaining original token types, solver relaxations and report identity.

use ospf_rust_bridge::{JobView, Result, SolveRequest, decode_compiled_model};
use crate::{NativeEngine, create_solver};

impl NativeEngine {
    pub(crate) fn start_compiled_linear(&self, request: SolveRequest) -> Result<JobView> {
        let model = decode_compiled_model(&request.model)?;
        let solver = create_solver(&request.solver, request.tuning_options.as_ref())?;
        // 仅此显式子模型 LP 入口采用保留对偶的 SCIP 预设，普通线性入口不变。
        // Only this explicit child-LP entry uses SCIP defaults retaining duals; ordinary linear execution stays unchanged.
        #[cfg(feature = "scip")]
        let solver: std::sync::Arc<dyn ospf_rust_core::solver::Solver> = if request.solver == "scip" && !model.has_integer_variables() {
            use ospf_rust_core_plugin_scip::{SCIPConfig, SCIPSolver};
            std::sync::Arc::new(SCIPSolver::with_config(SCIPConfig::default().with_output(false).with_lp_subproblem_defaults()))
        } else { solver };
        self.start(request, solver, model)
    }
}

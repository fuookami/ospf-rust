//! 实际提交模型的预取消身份绑定，不覆盖已有身份、不签发证明。 / Pre-cancel identity binding for the actual submitted model, without overwriting identities or issuing proofs.

use ospf_rust_core::solver::{SolveReport, TerminationReason, linear_model_fingerprint, linear_model_mapping};

pub(crate) fn bind_cancelled_identity(report: &mut SolveReport<f64>, model: &ospf_rust_core::model::intermediate::LinearTriadModel) -> ospf_rust_core::Result<()> {
    // 后端在预取消时可返回无模型元数据的通用报告；仅补实际提交子模型身份，不覆盖冲突身份或签发证明。
    // A backend can return a generic pre-cancel report; bind only the actual submitted child's identity, without overwriting conflicts or issuing proofs.
    if !report.has_incumbent() && report.proof.is_none() && matches!(report.termination_reason, TerminationReason::Cancelled | TerminationReason::Interrupted) {
        if report.fingerprints.model.is_none() { report.fingerprints.model = Some(linear_model_fingerprint(model)?); }
        if report.model_mapping.is_none() { report.model_mapping = Some(linear_model_mapping(model)?); }
    }
    Ok(())
}

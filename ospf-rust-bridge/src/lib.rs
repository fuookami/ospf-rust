//! 跨语言桥接协议与模型校验 / Language bridge protocol and model validation.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use ospf_rust_core::model::{ConstraintRelation, MetaModel, ObjectiveCategory};
use ospf_rust_core::model::intermediate::LinearTriadModel;
use ospf_rust_core::solver::linear_model_fingerprint;
use ospf_rust_core::variable::{BinaryVariableItem, ContinuousVariableItem, IntegerVariableItem, VariableRange};
use ospf_rust_framework::solver::remote::{
    SerializedConstraintSign, SerializedLinearModel, SerializedObjectiveCategory,
    SerializedVariable, SerializedVariableType, serialize_linear_model,
};

mod quadratic;
mod cp;
mod analysis_pipeline;
mod cp_analysis;
mod cp_options;
mod combinatorial;
mod fixed_recourse;
mod column_generation;
mod quantities;
mod bpp1d;
mod bpp2d;
mod bpp2d_execution;
mod bpp2d_packing;
mod bpp3d;
mod source_model;
mod linear_report;
mod pricing;
mod sweep;
mod network_flow;
mod network_generalized_flow;
mod network_design;
mod network_structure;
mod network_allocation;
mod network_resilience;
mod network_vrp;
mod quadratic_orchestration;
mod column_certificate;
mod remote_codec;
mod analysis_conflict;
mod analysis_funnel;
mod analysis_effectiveness;
mod analysis_report;
mod analysis_correction;
mod analysis_fixed_integer;
mod analysis_perturbation;
mod analysis_target;
mod analysis_multi_target;
mod analysis_criticality;
mod hyper_heuristic_config;
mod hyper_heuristic_reward;
mod hyper_heuristic_checkpoint;
mod linear_mapping;
mod lbb_resume_identity;
mod cp_checkpoint;
mod benders_metrics;
mod benders_cut_report;
mod symbol_condition;
mod symbol_linearization;
mod symbol_core_relations;
mod heuristic_schedule;
mod q_table;
mod master_cut_encoding;
mod analysis_group_aggregate;
mod predicate_field_resolution;
mod predicate_field_filter_extract;
mod model_configuration;
mod cache_key_identity;
mod cache_key_address;
mod flatten_cache;
mod update_assignment;
mod boolean_scope;
mod unsupported_predicate;
mod repository_calls;
mod function_usage;
mod input_domain;
mod native_write_record;
mod elastic_model;
mod combine_status;
mod number_text;
mod big_m_policy;
mod conditional_semantics;
mod search_budget;
mod predicate_schema;
mod sort_by;
mod variable_kind_domain;
mod critical_assembly;
mod flatten_identity;
mod master_cut;
mod boolean_dsl_fold;
mod field_resolution_states;
mod scalar_function_vocabulary;
mod polynomial_lift;
mod bounded_no_good_family;
mod effectiveness_aggregate;
mod assumption_binding;
mod tier_classification;
mod polarity_encoding;
mod quadratic_monomial;
mod deferred_materialize;
mod backend_features;
mod canonical_monomial;
mod expression_key_encoding;
mod encoding_label;
mod activity_bands;
mod evidence_predicates;
mod activity_report_check;
mod auxiliary_domain;
mod binding_assignment;
mod source_identity;
mod cut_validity;
mod assignment_composition;
mod dump_options;
mod native_write_scope;
mod writer_registry;
mod candidate_shortlist;
mod lazy_flatten_init;
mod dump_batch_dispatch;
mod quadratic_elastic;
mod lbb_stable_value;
mod master_assignment_extract;
mod sparse_triad_probe;
mod sparse_matrix_rows;
mod polynomial_probe;
mod model_view_probe;
mod analysis_options;
mod capability_matrix_probe;
mod cp_support_merge;
mod subproblem_result_probe;
mod no_good_oracle_probe;
mod cp_model_build_probe;
mod snapshot_gate_probe;
mod constraint_canon_probe;
mod assignment_eval_probe;
mod expression_bounds_probe;
mod domain_probe;
mod identity_wrap_probe;
mod container_surface_probe;
mod activation_set_probe;
mod tier_selection_probe;
mod evidence_remap_probe;
mod dominance_probe;
mod dispersion_probe;
mod reference_points_probe;
mod pareto_archive_probe;
mod constraint_ranking_probe;
mod constraint_priority_probe;
mod population_config_probe;
mod boundary_probe;
mod acceptance_probe;
mod ranking_probe;
mod topology_probe;
mod migration_probe;
mod archive_probe;
mod piecewise_probe;
mod diversity_probe;
mod solution_status_probe;
mod funnel_probe;
mod effectiveness_probe;
mod critical_report_probe;
mod group_summary_probe;
mod band_probe;
mod evidence_probe;
mod proof_probe;
mod gap_probe;
mod diagnostics_probe;
mod fingerprint_probe;
mod blocking_set_probe;
mod correction_probe;
mod sensitivity_probe;
mod funnel_ranking_probe;
mod effectiveness_ranking_probe;
mod persistence_sort_probe;
mod persistence_schema_probe;
mod predicate_scope_probe;
mod native_terminal_proof_probe;
mod native_lowering_probe;
mod solve_value_probe;
mod selection_policy_probe;
mod non_stationary_probe;
mod training_split_probe;
mod acceptance_recovery_probe;
mod hypervolume_probe;
mod operator_capability_probe;
mod schedule_log_probe;
mod indexed_registration;
mod heartbeat_response;
mod dynamic_lifecycle;
mod logic_based_benders;
mod relational_query;
mod linear_benders;
mod predicate_filters;
mod persistence_edit;
mod repository_query;
mod pipeline_plan;
mod bpp_loading;
pub use bpp_loading::*;
mod csp1d;
mod csp1d_report;
mod csp1d_production_report;
mod csp1d_execution;
mod csp2d;
mod csp2d_execution;
mod geometry2d;
pub use geometry2d::*;
pub use quantities::{PhysicalQuantity, QuantityUnit};
pub use bpp1d::*;
pub use bpp2d::*;
pub use bpp2d_execution::*;
pub use bpp2d_packing::*;
mod gantt_activity;
pub use gantt_activity::*;
pub use bpp3d::*;
pub use csp1d::*;
pub use csp1d_report::*;
pub use csp1d_production_report::*;
pub use csp1d_execution::*;
pub use csp2d::*;
pub use csp2d_execution::*;
pub use column_generation::{ColumnGenerationBlock, ColumnGenerationOptions, ColumnGenerationReport, ColumnGenerationStop, ColumnGenerationSummary, decode_column_generation_report, encode_column_generation_result};
pub use cp::{ActivityOptions, ActivityStatus, ConstraintOptions, decode_cp_report, decode_cp_snapshot, encode_cp_snapshot};
pub use pricing::{CertifiedPricingDual, LinearPricingDuals, verify_pricing_duals};
pub use sweep::{LinearSweepCase, LinearSweepCaseResult, LinearSweepReport};
pub use pipeline_plan::{PipelineGroupConflict, PipelineGroupSpec, PipelinePlanReport, PipelineStepSpec, PipelineStepVerdict, plan_pipeline_registration};
pub use repository_query::{RepositoryQuerySpec, build_repository_query, effective_paging};
pub use persistence_edit::{AssignmentEntry, AssignmentKind, AssignmentReport, AssignmentSpec, AssignmentValue, NullsSupportLevel, SortItemVerdict, SortPlanReport, SortPlanSpec, validate_assignments, validate_sort_plan};
pub use predicate_filters::{FieldFilterEntry, FieldFilterReport, FieldFilterSpec, extract_field_filters};
pub use benders_cut_report::{verify_benders_cuts, BendersCutReport, BendersCutRequest, BendersCutRow};
pub use benders_metrics::{verify_benders_metrics, BendersMetricsReport, BendersMetricsRequest, BendersSnapshotInput, BendersSnapshotRow};
pub use schedule_log_probe::{verify_schedule_log, ScheduleEventSpec, ScheduleLogReport, ScheduleLogRequest, ScheduleLogRow, ScheduleLogSpec};
pub use operator_capability_probe::{verify_operator_capability, OperatorCapabilityReport, OperatorCapabilityRequest, OperatorRegistrationRow, OperatorRegistrationSpec};
pub use hypervolume_probe::{verify_hypervolume_probe, HypervolumeMeasurementSpec, HypervolumeProbeReport, HypervolumeProbeRequest, HypervolumeMeasureRow};
pub use acceptance_recovery_probe::{verify_acceptance_recovery, AcceptanceDefinitionSpec, AcceptanceLoadRow, AcceptanceOverrideSpec, AcceptanceRecoveryReport, AcceptanceRecoveryRequest};
pub use training_split_probe::{verify_training_split_probe, TrainingCaseSpec, TrainingParameterSpec, TrainingSplitProbeReport, TrainingSplitProbeRequest};
pub use non_stationary_probe::{verify_non_stationary_probe, NonStationaryObservationSpec, NonStationaryProbeReport, NonStationaryProbeRequest, NonStationarySelectSpec};
pub use selection_policy_probe::{verify_selection_probe, SelectionObservationSpec, SelectionProbeReport, SelectionProbeRequest, SelectionStepSpec};
pub use solve_value_probe::{verify_solve_value_probe, SolveValueProbeReport, SolveValueProbeRequest};
pub use native_lowering_probe::{verify_native_lowering_probe, NativeLoweringProbeReport, NativeLoweringProbeRequest, NativeOutcomeSpec};
pub use native_terminal_proof_probe::{verify_native_terminal_proof_probe, NativeTerminalProofReport, NativeTerminalProofRequest};
pub use predicate_scope_probe::{verify_predicate_scope_probe, PredicateScopeReport, PredicateScopeRequest, PredicateScopeSpec};
pub use persistence_schema_probe::{verify_persistence_schema_probe, PersistenceSchemaReport, PersistenceSchemaRequest, PersistenceFieldSpec, SchemaQueryRow};
pub use persistence_sort_probe::{verify_persistence_sort_probe, PersistenceSortItemSpec, PersistenceSortProbeReport, PersistenceSortProbeRequest, PersistenceSortRow};
pub use effectiveness_ranking_probe::{verify_effectiveness_ranking_probe, EffectivenessEntryRow, EffectivenessEntrySpec, EffectivenessRankingReport, EffectivenessRankingRequest};
pub use funnel_ranking_probe::{verify_funnel_probe, FunnelCandidateRow, FunnelSortCandidateSpec, FunnelProbeReport, FunnelProbeRequest};
pub use sensitivity_probe::{verify_sensitivity_probe, LocalSensitivitySpec, SensitivityProbeReport, SensitivityProbeRequest};
pub use correction_probe::{verify_correction_probe, CorrectionCandidateSpec, CorrectionMemberRow, CorrectionProbeReport, CorrectionProbeRequest};
pub use blocking_set_probe::{verify_blocking_set, BlockingSetReport, BlockingSetRequest, VerificationSpec};
pub use fingerprint_probe::{verify_fingerprint_probe, AuditFingerprintSpec, FingerprintProbeReport, FingerprintProbeRequest};
pub use diagnostics_probe::{verify_diagnostics_probe, DiagnosticsProbeReport, DiagnosticsProbeRequest, EvaluationSpec, MessageSpec};
pub use gap_probe::{verify_gap_probe, GapProbeReport, GapProbeRequest, SnapshotSpec};
pub use proof_probe::{verify_proof_probe, ProofProbeReport, ProofProbeRequest, ProofSpec};
pub use evidence_probe::{verify_evidence_probe, EvidenceProbeReport, EvidenceProbeRequest, InfeasibilityEvidenceSpec};
pub use band_probe::{verify_band_probe, BandProbeReport, BandProbeRequest, BandProbeRow, BandProbeSpec};
pub use group_summary_probe::{verify_group_summary, GroupActivitySpec, GroupEffectivenessSpec, GroupSummaryReport, GroupSummaryRequest, GroupSummaryRow};
pub use critical_report_probe::{verify_critical_report, CriticalReportRequest, CriticalReportVerdict};
pub use effectiveness_probe::{verify_effectiveness_probe, EffectivenessProbeReport, EffectivenessProbeRequest, EffectivenessProbeRow, EffectivenessProbeSpec};
pub use funnel_probe::{verify_funnel, FunnelProbeSpec, FunnelReport, FunnelRequest, FunnelRow};
pub use solution_status_probe::{verify_solution_status, SolutionStatusReport, SolutionStatusRequest};
pub use diversity_probe::{verify_diversity, DiversityReport, DiversityRequest};
pub use piecewise_probe::{verify_piecewise, PiecewisePointSpec, PiecewiseReport, PiecewiseRequest};
pub use archive_probe::{verify_archive_probe, ArchiveProbeReport, ArchiveProbeRequest, ArchiveStepRow};
pub use migration_probe::{verify_migration, MigrationReport, MigrationRequest};
pub use topology_probe::{verify_topology, TopologyReport, TopologyRequest};
pub use ranking_probe::{verify_ranking, RankingItemRow, RankingProbeReport, RankingProbeRequest};
pub use acceptance_probe::{verify_acceptance, AcceptanceReport, AcceptanceRequest};
pub use boundary_probe::{verify_boundary, BoundaryReport, BoundaryRequest};
pub use population_config_probe::{verify_population_config, PopulationConfigReport, PopulationConfigRequest};
pub use constraint_priority_probe::{verify_constraint_priority, PriorityReport, PriorityRequest, PriorityView};
pub use constraint_ranking_probe::{verify_constraint_ranking, ConstraintRankingReport, ConstraintRankingRequest, RankingAssessmentSpec, RankingEntryView};
pub use pareto_archive_probe::{verify_pareto_archive, ArchiveReport, ArchiveRequest};
pub use reference_points_probe::{verify_reference_points, ReferencePairSpec, ReferencePairView, ReferencePointsReport, ReferencePointsRequest};
pub use dispersion_probe::{verify_dispersion, CrowdingPointView, DispersionFrontView, DispersionReport, DispersionRequest};
pub use dominance_probe::{verify_dominance, DominancePairSpec, DominancePairView, DominanceReport, DominanceRequest};
pub use evidence_remap_probe::{verify_evidence_remap, EvidenceMemberSpec, EvidenceRemapReport, EvidenceRemapRequest, EvidenceSourceSpec, EvidenceSourceVariableSpec, EvidenceSourceView};
pub use tier_selection_probe::{verify_tier_selection, TierOverrideSpec, TierSelectionReport, TierSelectionRequest};
pub use activation_set_probe::{verify_activation_set, ActivationOpSpec, ActivationRecordView, ActivationSetReport, ActivationSetRequest, ActivationSourceSpec, ActivationVariableSpec};
pub use container_surface_probe::{verify_container_surface, ContainerProbeReport, ContainerProbeRequest, ContainerTermSpec, ContainerView};
pub use identity_wrap_probe::{verify_identity_wrapping, FingerprintSpec, IdentityWrapReport, IdentityWrapRequest, IdentityWrapView};
pub use domain_probe::{verify_domain_surface, DomainProbeReport, DomainProbeRequest, DomainProbeSpec, DomainView};
pub use expression_bounds_probe::{verify_expression_bounds, ExpressionBoundsReport, ExpressionBoundsRequest, ExpressionDomainSpec, ExpressionTermSpec, ExpressionTermView};
pub use assignment_eval_probe::{verify_assignment_evaluation, AssignmentEvalReport, AssignmentEvalRequest, AssignmentValueSpec, IntervalValueView};
pub use constraint_canon_probe::{verify_constraint_canonicalization, ConstraintCanonReport, ConstraintCanonView, ConstraintCanonRequest, ConstraintShapeSpec};
pub use snapshot_gate_probe::{verify_snapshot_gates, SnapshotGateReport, SnapshotGateRequest, SnapshotValueSpec};
pub use cp_model_build_probe::{verify_cp_model_build, CpConstraintSpec, CpGroupSpec, CpModelBuildReport, CpModelBuildRequest, CpVariableSpec};
pub use no_good_oracle_probe::{verify_no_good_oracle, NoGoodOracleReport, NoGoodOracleRequest, OracleAssignmentEntry, OracleBindingLiteral, OracleCutView};
pub use subproblem_result_probe::{verify_subproblem_result, SubproblemResultReport, SubproblemResultRequest, SubproblemValueSpec};
pub use cp_support_merge::{verify_cp_support_merge, CpConstraintLevelSpec, CpFeatureRow, CpSupportMergeReport, CpSupportMergeRequest};
pub use capability_matrix_probe::{verify_capability_matrix, CapabilityMatrixReport, CapabilityMatrixRequest, CapabilityRow};
pub use analysis_options::{verify_analysis_options, AnalysisOptionsReport, AnalysisOptionsRequest};
pub use model_view_probe::{verify_model_views, ModelViewReport, ModelViewRequest};
pub use polynomial_probe::{verify_polynomial_probe, LinearTermRow, PolynomialTermSpec, PolynomialProbeReport, PolynomialProbeRequest};
pub use sparse_matrix_rows::{verify_matrix_rows, MatrixEntrySpec, MatrixRowSpec, MatrixRowsReport, MatrixRowsRequest};
pub use sparse_triad_probe::{verify_sparse_triad, SparseBoundsRow, SparseBoundsSpec, SparseEntrySpec, SparseTriadReport, SparseTriadRequest};
pub use master_assignment_extract::{verify_master_assignment, MasterAssignedValue, MasterAssignmentReport, MasterAssignmentRequest, MasterAssignedVariableSpec};
pub use lbb_stable_value::{verify_stable_value, StableValueEntry, StableValueReport, StableValueRequest};
pub use quadratic_elastic::{verify_quadratic_elastic, QuadraticElasticReport, QuadraticElasticRequest};
pub use dump_batch_dispatch::{verify_dump_batch, DumpBatchReport, DumpBatchRequest, DumpFileRow};
pub use lazy_flatten_init::{verify_lazy_flatten_init, LazyInitReport, LazyInitRequest};
pub use candidate_shortlist::{verify_candidate_shortlist, CandidateEntrySpec, ShortlistReport, ShortlistRequest};
pub use writer_registry::{verify_writer_registry, DestinationRow, WriterRegistryReport, WriterRegistryRequest, WriterSpec};
pub use native_write_scope::{verify_native_write_scope, NativeWriteScopeReport, NativeWriteScopeRequest, OutcomePredicates, OutcomeSpec, NativeStructureSpec, WriteRecordSpec};
pub use dump_options::{verify_dump_options, DumpOptionsReport, DumpOptionsReportDefaults, DumpOptionsSpec, FixedVariableSpec};
pub use assignment_composition::{verify_assignment_composition, AssignLiteral, AssignOutcome, AssignStep, AssignmentCompositionReport, AssignmentCompositionRequest, PathOutcome};
pub use cut_validity::{verify_cut_validity, CutValidityOutcome, CutValidityReport, CutValidityRequest, CutValiditySpec};
pub use source_identity::{verify_source_identity, SourceIdentity, SourceIdentityReport, SourceIdentityRequest, SourceSpec};
pub use binding_assignment::{verify_binding_assignment, BindingAssignmentReport, BindingAssignmentRequest, BindingLiteral};
pub use auxiliary_domain::{verify_auxiliary_domain, AuxiliaryDomainReport, AuxiliaryDomainRequest, AuxiliaryOutcome, AuxiliarySpec};
pub use activity_report_check::{verify_activity_report, ActivityReportRequest, ActivityReportVerdict, BoundRecordSpec, ConstraintRecordSpec, GroupSummarySpec, StatusCountSpec};
pub use evidence_predicates::{verify_evidence_predicates, EvidenceReport, EvidenceRow, EvidenceSpec, EvidenceSpecRequest};
pub use activity_bands::{verify_activity_bands, ActivityBandReport, ActivityBandRequest, ActivityBandRow};
pub use encoding_label::{verify_encoding_label, EncodingAssignmentRow, EncodingLabelReport, EncodingLabelRequest, EncodingTerm};
pub use expression_key_encoding::{verify_expression_key, ExpressionKeyReport, ExpressionKeyRequest, KeyTerm};
pub use canonical_monomial::{verify_canonical_monomial, CanonicalMonomialReport, CanonicalMonomialRequest, PowerRow, PowerEntry};
pub use backend_features::{verify_backend_features, BackendFeaturesReport, BackendFeaturesRequest, CompileTimeSnapshot};
pub use deferred_materialize::{verify_materialize, MaterializeReport, MaterializeRequest, StructureSpec, StructureTrace};
pub use quadratic_monomial::{verify_quadratic_monomial, MonomialReport, MonomialSpec};
pub use polarity_encoding::{verify_polarity_encoding, PolarityReport, PolarityRequest, BinaryVariableSpec, IntegerVariableSpec, SparseTerm, RowClosedForm, FamilyRowForms};
pub use tier_classification::{verify_tier_classification, TierClassificationReport, TierClassificationRequest, CandidateSpec, TierRow};
pub use assumption_binding::{verify_assumption_binding, AssumptionBindingReport, AssumptionBindingRequest, AssumptionRow, AssumptionRow2, AssumptionSpec, AssumptionMappingSpec};
pub use effectiveness_aggregate::{verify_effectiveness_aggregate, EffectivenessAggregateReport, EffectivenessAggregateRequest, AggregateRow, ObservationSpec, PerturbationSpec, FunnelCandidateSpec};
pub use bounded_no_good_family::{verify_bounded_no_good, BoundedNoGoodReport, BoundedNoGoodRequest, BoundedNoGoodRow, BoundedNoGoodVariableSpec, BoundedNoGoodValue, VerifiedConflictRow};
pub use polynomial_lift::{verify_polynomial_lift, PolynomialLiftReport, PolynomialLiftRequest, LinearTermSpec, PowerSpec};
pub use scalar_function_vocabulary::{verify_scalar_functions, ScalarFunctionReport, ScalarFunctionRequest, ScalarFunctionRow};
pub use field_resolution_states::{verify_field_resolution, FieldResolutionReport, FieldResolutionRequest};
pub use boolean_dsl_fold::{verify_boolean_fold, BooleanFoldReport, BooleanFoldRequest};
pub use master_cut::{verify_master_cut, MasterCutReport, MasterCutRequest, MasterCutTerm, MasterCutTermRow};
pub use flatten_identity::{verify_flatten_identity, FlattenIdentityReport, FlattenIdentityRequest};
pub use critical_assembly::{verify_critical_assembly, AssemblyCase, AssemblyRow, CriticalAssemblyReport, CriticalAssemblyRequest};
pub use variable_kind_domain::{verify_variable_kind_domain, VariableKindDomainReport, VariableKindDomainRequest, VariableKindRow};
pub use sort_by::{verify_sort_by, SortByReport, SortByRequest, SortItemSpec, SortedItemRow, SupportRow};
pub use predicate_schema::{verify_predicate_schema, PredicateSchemaReport, PredicateSchemaRequest, ResolveRow, SameFieldRow, SchemaFieldSpec};
pub use search_budget::{verify_search_budget, RandomVerdict, SearchBudgetReport, SearchBudgetRequest, SliceVerdict};
pub use conditional_semantics::{verify_conditional_semantics, AliasRow, BoundsProbeRow, ClassificationRow, ConditionalSemanticsReport, ConditionalSemanticsRequest, ConditionBoundsSpec, ConditionPolynomialSpec, EntryShapeRow, RelationSemanticsRow};
pub use big_m_policy::{verify_big_m_policy, BigMIndicatorRow, BigMPolicyReport, BigMPolicyRequest, BigMPolynomialSpec, BigMProbeRow};
pub use number_text::{verify_number_text, NumberTextReport, NumberTextRequest, NumberTextRow, TextFloatSpec};
pub use combine_status::{verify_combine_status, CombineCell, CombineStatusReport, CombineStatusRequest};
pub use elastic_model::{verify_elastic_model, ElasticModelReport, ElasticModelRequest, KindPair, ModelRowSpec, ModelVariableSpec, SlackRow, TransformRow};
pub use native_write_record::{verify_native_write_record, BindingRow, BindingSpec, FingerprintCase, NativeWriteReport, NativeWriteRequest, ProofPair, ProofRow, WriteFingerprintRow};
pub use input_domain::{verify_input_domain, DomainPairRow, DomainPairSpec, DomainRow, DomainSpec, FingerprintRow, FloatSpec, InputDomainReport, InputDomainRequest};
pub use function_usage::{verify_function_usage, FunctionUsageReport, FunctionUsageRequest, UsageFlags, UsageRow};
pub use repository_calls::{verify_repository_calls, RepositoryCallReport, RepositoryCallRequest, RepositoryCallRow};
pub use unsupported_predicate::{verify_unsupported_predicate, AnnotationSpec, PolicyAlias, PolicyRow, UnsupportedPredicateReport, UnsupportedPredicateRequest};
pub use boolean_scope::{verify_boolean_scope, BooleanScopeReport, BooleanScopeRequest, ScopeStep, ScopeStepRow, ScopeValue};
pub use update_assignment::{verify_update_assignments, AssignmentExpressionSpec, AssignmentGroup, AssignmentRow, UpdateAssignmentReport, UpdateAssignmentRequest, UpdateAssignmentSpec, UpdateAssignmentValue};
pub use flatten_cache::{verify_flatten_cache, FlattenCacheOperation, FlattenCacheReport, FlattenCacheRequest, FlattenCacheRow, FlattenSymbolSpec};
pub use cache_key_address::{verify_cache_key_address, CacheKeyAddressReport, CacheKeyAddressRequest, EqualValueGroup};
pub use cache_key_identity::{verify_cache_key_identity, CacheKeyIdentityReport, CacheKeyIdentityRequest, CacheOperationRow, CacheOperationSpec, RangeKeySpec, ValueKeySpec};
pub use model_configuration::{verify_model_configuration, BuildingStatusRow, BuildingStatusSpec, ModelConfigurationDefaults, ModelConfigurationReport, ModelConfigurationRequest};
pub use predicate_field_filter_extract::{verify_predicate_field_filters, EqFilterRow, EqOrInRow, FieldFilterRow, FilterConditionSpec, FilterValueSpec, PredicateFieldFilterReport, PredicateFieldFilterRequest};
pub use predicate_field_resolution::{verify_predicate_field_resolution, PredicateFieldMapping, PredicateFieldProbeReport, PredicateFieldResolutionReport, PredicateFieldResolutionRequest};
pub use analysis_group_aggregate::{aggregate_analysis_groups, AnalysisGroupAggregateReport, AnalysisGroupAggregateRequest, AnalysisGroupRow};
pub use master_cut_encoding::{verify_master_cut_encoding, MasterAssignmentEntry, MasterCutEncodingReport, MasterCutEncodingRequest, MasterNoGoodRowSpec};
pub use q_table::{verify_q_table, QSelectionOutcome, QSelector, QTableReport, QTableRequest, QTransition, QTransitionOutcome};
pub use heuristic_schedule::{sample_heuristic_schedule, HeuristicScheduleReport, HeuristicScheduleRequest, HeuristicScheduleSample};
pub use symbol_core_relations::{verify_symbol_core_relations, SymbolCoreRelation, SymbolCoreRelationsRequest, SymbolCoreRelationsReport, SymbolImplyRow, SymbolInequalityRow, SymbolInValuesTable};
pub use symbol_linearization::{verify_symbol_linearization, SymbolBinaryzationRow, SymbolLinearizationReport, SymbolLinearizationRequest};
pub use symbol_condition::{verify_symbol_condition, SymbolConditionBounds, SymbolConditionReport, SymbolConditionRequest, SymbolConditionSample};
pub use cp_checkpoint::{verify_cp_checkpoint, CpCheckpointReport, CpCheckpointRequest};
pub use lbb_resume_identity::{verify_lbb_resume_identity, CancellationInput, FingerprintInput, LbbResumeIdentityReport, LbbResumeIdentityRequest, ProvenanceInput};
pub use linear_mapping::{verify_linear_mapping, LinearMappingReport, LinearMappingRequest};
pub use hyper_heuristic_checkpoint::{verify_checkpoint, CheckpointReport, CheckpointRequest};
pub use hyper_heuristic_reward::{verify_reward_policy, ExtremeValueInput, HypervolumeConfigInput, HypervolumeRow, RewardPolicyReport, RewardPolicyRequest, TimeNormalizedInput};
pub use hyper_heuristic_config::{AlgorithmRow, BudgetRow, ExecutionRow, HyperHeuristicConfigReport, HyperHeuristicConfigRequest, LearningRow, MemoryRow, PersistenceRow, verify_hyper_heuristic_config};
pub use analysis_criticality::{CriticalityObservationInput, CriticalityObservationRow, CriticalityProfileReport, CriticalityRequest, verify_criticality_profile};
pub use analysis_multi_target::{BottleneckKindRow, MultiTargetReportSummary, MultiTargetResultRow, verify_multi_target_report};
pub use analysis_target::{TargetEcho, TargetReportSummary, TargetSpec, TargetVerification, verify_target};
pub use analysis_perturbation::{AdaptivePerturbationEcho, AdaptivePerturbationInput, PerturbationObservationRow, PerturbationPolicyRequest, PerturbationReportSummary, PerturbationVerification, verify_perturbation_policy};
pub use analysis_fixed_integer::{FixedIntegerConfigInput, FixedIntegerLpReport, FixedIntegerSensitivityRow, verify_fixed_integer_lp};
pub use analysis_correction::{RelaxationCostInput, RelaxationPolicyInput, RelaxationRequest, RelaxationMember, RelaxationPlan, RelaxationReport, generate_relaxation_recommendations};
pub use analysis_report::{CriticalAnalysisReport, CriticalBaseline, CriticalStagePresence, verify_critical_analysis};
pub use analysis_effectiveness::{EffectivenessReport, EffectivenessRow, verify_effectiveness_ranking};
pub use analysis_funnel::{CandidateFunnelReport, ShortlistEntry, verify_candidate_funnel};
pub use analysis_conflict::{ConflictExplanationSummary, ConflictGroupEntry, verify_conflict_explanation};
pub use remote_codec::{RemoteArtifactKind, RemoteArtifactReport, RemoteArtifactRequest, RemoteCheckpointIdentity, RemoteCodecOperation, RemoteFingerprintSpec, RemoteResumeExpectation, decode_remote_artifact};
pub use linear_benders::{BendersCutCoefficient, BendersCutSense, BendersCutSpec, BendersMasterReport, BendersQuadraticCutSpec, BendersQuadraticTerm, BendersSubOutcome, BendersSubReport, solve_benders_master, solve_benders_quadratic_master, solve_benders_sub, to_linear_cuts, to_quadratic_cuts};
pub use relational_query::{QueryColumn, QueryComparison, QueryFilter, QueryJoinCardinality, QueryJoinSpec, QueryJoinType, QueryNullsOrder, QueryOrderSpec, QueryPageSpec, QueryPlanLimits, QueryPlanReport, QueryPlanViolation, QueryScalar, QuerySortDirection, QuerySourceSpec, QueryTrivalent, RelationalQueryPlanSpec, validate_query_plan};
pub use logic_based_benders::{CpAssumptionSpec, LogicBasedBendersAssumptionBinding, LogicBasedBendersPlanReport, LogicBasedBendersPlanRequest, LogicBasedBendersReport, LogicBasedBendersRequest, MasterAssignmentValue, MasterCutCoefficient, MasterCutScope, MasterCutSense, MasterCutSpec, MasterVariableDomain, MasterVariableSpec, assumption_bindings, extract_master_assignment, plan_logic_based_benders, solve_logic_based_benders, validate_logic_based_benders, apply_iteration_budget};
pub use dynamic_lifecycle::{LinearColumnOperation, LinearColumnOperationKind, LinearColumnState, LinearColumnStateEntry, LinearLifecycleReport, LinearLifecycleRequest, apply_lifecycle, column_range_override};
pub use heartbeat_response::{HeartBeatRequest, HeartBeatSnapshot, ResponseEnvelope, observe_heartbeat};
pub use indexed_registration::{IndexedKey, IndexedKeyIndex, IndexedRegistrationMode, IndexedRegistrationReport, IndexedRegistrationRequest, IndexedVariableKind, register_indexed_keys};
pub use column_certificate::{LinearColumnBatch, LinearColumnCertificate, LinearColumnCompletion, LinearColumnScope, verify_column_batch};
pub use quadratic_orchestration::{QuadraticOrchestration, QuadraticOrchestrationOptions, QuadraticOrchestrationReport, CoreQuadraticBackend, core_backend, decode_quadratic_orchestration_report, encode_quadratic_orchestration_report, quadratic_identity, solve_quadratic_orchestration};
pub use network_vrp::{VrpArcSpec, VrpAxis, VrpCustomerSpec, VrpDepotSpec, VrpNodeSpec, VrpProblem, VrpReport, VrpRoute, VrpSearchOptions, VrpSolution, VrpStop, VrpTimeWindow, VrpTolerancesSpec, VrpUnitsSpec, VrpVehicleTypeSpec, decode_vrp_solution, summarize_vrp_report};
pub use network_resilience::{FlowRadiusValue, NetworkResilienceProblem, NetworkResilienceReport, NetworkResilienceSolution, ResilienceArcQuantity, ResilienceCommodityBalances, ResilienceCommoditySlacks, ResilienceConnectivity, ResilienceConnectivityFlow, ResilienceDisjointness, ResilienceEffectiveCapacities, ResilienceNodeSlack, ResilienceScenario, ResilienceScenarioResult, decode_network_resilience_solution, summarize_network_resilience_report};
pub use network_allocation::{AllocationFlowQuantity, AllocationHubSpec, AllocationLocationSpec, AllocationNamedQuantity, AllocationOptionsSpec, AllocationProblem, AllocationQuotaSpec, AllocationReport, AllocationRequestSpec, AllocationServiceSpec, AllocationShipment, AllocationSolution, decode_allocation_solution, summarize_allocation_report};
pub use network_structure::{NetworkParentArc, NetworkPartitionOwner, NetworkStructurePartition, NetworkStructureProblem, NetworkStructureQuota, NetworkStructureReport, NetworkStructureSolution, decode_network_structure_solution, summarize_network_structure_report};
pub use network_design::{NetworkConstruction, NetworkDesignFacility, NetworkDesignLevel, NetworkDesignProblem, NetworkDesignReport, NetworkDesignService, NetworkDesignSolution, NetworkServiceLevel, decode_network_design_solution, summarize_network_design_report};
pub use network_generalized_flow::{GeneralizedFlowArc, GeneralizedFlowArcResult, GeneralizedFlowNode, GeneralizedFlowNodeResult, GeneralizedFlowProblem, GeneralizedFlowReport, GeneralizedFlowSolution, decode_generalized_flow_solution, summarize_generalized_flow_report};
pub use network_flow::{NetworkFlowArc, NetworkFlowArcFlow, NetworkFlowArcQuantity, NetworkFlowCommodity, NetworkFlowNamedQuantity, NetworkFlowNode, NetworkFlowNodeQuantity, NetworkFlowProblem, NetworkFlowQuantity, NetworkFlowReport, NetworkFlowSolution, decode_network_flow_solution, summarize_flow_report};
pub use analysis_pipeline::{AnalysisCandidate, AnalysisCandidateFunnel, AnalysisCandidateTier, AnalysisFunnelOptions, LinearIisAlgorithm, LinearIisOptions, LinearIisReport, LinearSolutionPoolOptions, LinearSolutionPoolReport, build_analysis_candidate_funnel, compute_linear_iis};
pub use cp_analysis::{MipLoweringAnalysis, analyze_cp_mip_lowering};
pub use cp_options::{ConstraintProgrammingOptions, IntegerSolutionHint};
pub use combinatorial::{NativeBackend, ParallelCombinatorialMode, ParallelLinearOptions, SerialFallbackPolicy, SerialLinearOptions, decode_combinatorial_report};
pub use fixed_recourse::{FixedRecourseOptions, FixedRecourseReport, FixedRecourseStage, FixedRecourseStop, FixedRecourseSummary, StagedVariable, decode_fixed_recourse_report, encode_fixed_recourse_result, encode_fixed_recourse_snapshot_result};
pub use quadratic::{compile_quadratic, decode_quadratic_model, encode_quadratic_model, quadratic_objective_matrix};

/// 桥接主版本 / Bridge protocol major version.
pub const PROTOCOL_VERSION: u32 = 1;

/// 结构化边界错误 / Structured boundary error.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BridgeError {
    /// 稳定错误码 / Stable error code.
    pub code: String,
    /// 错误说明 / Error description.
    pub message: String,
}

impl BridgeError {
    /// 创建边界错误 / Create a boundary error.
    pub fn new(code: &str, message: impl ToString) -> Self {
        Self { code: code.into(), message: message.to_string() }
    }
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for BridgeError {}

/// 桥接结果 / Bridge result.
pub type Result<T> = std::result::Result<T, BridgeError>;

/// 无损模型封装，JSON 只由 Rust 解析 / Lossless model envelope, parsed only by Rust.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelArtifact {
    /// 模型 JSON 文本 / Model JSON text.
    pub model_json: String,
    /// codec 定义的模型／内容指纹；compiled-linear 与 core 报告身份分开。 / Codec-defined model/content fingerprint; compiled-linear identity is separate from the core report.
    pub fingerprint: String,
}

/// 执行模型类型，显式保留数值和模型语义。
/// Execution model kind, explicitly preserving numeric and model semantics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ModelKind {
    /// 线性或混合整数线性 / Linear or mixed-integer linear.
    Linear,
    /// 保留原 token 类型、松弛 solver 类型及元数据的实际已编译线性模型。 / Actual compiled linear model retaining original token types, relaxed solver types and metadata.
    CompiledLinear,
    /// 二次目标或二次约束，允许整数变量 / Quadratic objectives or constraints, including integer variables.
    Quadratic,
    /// 二次组合编排（多个已绑定二次后端串行或并行）；配置由 `quadraticOrchestration` 给出。
    /// Quadratic orchestration (several bound quadratic backends serially or in parallel); the configuration comes from
    /// `quadraticOrchestration`.
    QuadraticOrchestration,
    /// 真实线性模型的不可约不可行子系统（IIS）；原 core 的两种过滤算法都依赖 Gurobi 后端。
    /// Irreducible infeasible subsystem of a real linear model; both original core filters require the Gurobi backend.
    LinearIis,
    /// 线性/混合整数线性模型的解池；只有后端原生实现时才返回，否则显式拒绝。
    /// Solution pool of a linear/mixed-integer linear model; returned only when the backend implements it natively,
    /// otherwise rejected explicitly.
    LinearSolutionPool,
    /// 固定整数 LP 的局部有效性（敏感性）分析；需要先给出可行 incumbent 向量。
    /// Local-effectiveness (sensitivity) analysis of the fixed-integer LP; a feasible incumbent vector is required.
    LinearSensitivity,
    /// 同一模型的多后端/多预算扫描；每个 case 独立求解，结果按 case ID 稳定排序。
    /// Multi-backend/multi-budget sweep over one model; each case solves independently and results sort by case ID.
    LinearSweep,
    /// 原 `network-scheduling` 的多商品最小费用流；领域模型由 `networkFlowProblem` 给出。
    /// Multi-commodity min-cost flow from the original `network-scheduling` crate; the domain model comes from
    /// `networkFlowProblem`.
    NetworkFlow,
    /// 原 `network-scheduling` 的广义流（产率 + 单位转换）；领域模型由 `networkGeneralizedFlowProblem` 给出。
    /// Generalized flow (production yield plus unit conversion) from the original `network-scheduling` crate; the domain
    /// model comes from `networkGeneralizedFlowProblem`.
    NetworkGeneralizedFlow,
    /// 原 `network-scheduling` 的网络建设（共享建设选择 + 名义流）；领域模型由 `networkDesignProblem` 给出。
    /// Network design (shared construction choices plus nominal flow) from the original `network-scheduling` crate; the
    /// domain model comes from `networkDesignProblem`.
    NetworkDesign,
    /// 原 `network-scheduling` 的网络结构（根树 + 设施归属）；领域模型由 `networkStructureProblem` 给出。
    /// Network structure (rooted tree plus facility ownership) from the original `network-scheduling` crate; the domain
    /// model comes from `networkStructureProblem`.
    NetworkStructure,
    /// 原 `network-scheduling` 的运输分配（共享配额 + 枢纽建设）；领域模型由 `allocationProblem` 给出。
    /// Transportation allocation (shared quotas plus hub construction) from the original `network-scheduling` crate; the
    /// domain model comes from `allocationProblem`.
    NetworkAllocation,
    /// 原 `network-scheduling` 的韧性规划（显式故障场景 + 连通要求）；领域模型由 `networkResilienceProblem` 给出。
    /// Resilience planning (explicit failure scenarios plus connectivity requirements) from the original
    /// `network-scheduling` crate; the domain model comes from `networkResilienceProblem`.
    NetworkResilience,
    /// 按声明键注册变量并返回键 → 模型索引映射（覆盖 `optional_array`/`appendable_pool`/`indexed_combination`）。
    /// Register variables for declared keys and return the key → model-index map (covering `optional_array`,
    /// `appendable_pool` and `indexed_combination`).
    IndexedRegistration,
    /// 列生成批次证据复核（`column_generation_batch`）；空列不是证明。
    /// Column-generation batch evidence recheck (`column_generation_batch`); an empty pool is not a proof.
    LinearColumnCertificate,
    /// 运行心跳观测（`running_heart_beat`）。 / Running-heartbeat observation (`running_heart_beat`).
    HeartBeat,
    /// 通用响应信封（`network::Response`）。 / Generic response envelope (`network::Response`).
    ResponseEnvelope,
    /// 动态模型生命周期（`dynamic_lifecycle`）：列状态、解注入、warm start、flush。
    /// Dynamic model lifecycle (`dynamic_lifecycle`): column state, solution injection, warm start and flush.
    LinearLifecycle,
    /// 关系查询计划校验与规范化（`persistence::query` 的可移植子集）。
    /// Relational query plan validation and canonicalisation (the portable subset of `persistence::query`).
    RelationalQueryPlan,
    /// Benders 主问题单步（模型工件 + 显式 cuts → 统一报告）。
    /// Benders master single step (model artifact plus explicit cuts → unified report).
    LinearBendersMaster,
    /// Benders **二次**主问题单步（二次模型工件 + 显式线性/二次 cuts → 统一报告）。
    /// Benders **quadratic** master single step (quadratic model artifact plus explicit linear/quadratic cuts → report).
    QuadraticBendersMaster,
    /// Benders **子问题**单步（模型工件 + master 解向量 → 子问题结论与对偶）。
    /// Benders **subproblem** single step (model artifact plus the master solution → verdict and duals).
    LinearBendersSub,
    /// LBB 规划（请求校验 + assumption 绑定 + 可选按稳定 ID 的 assignment 提取）。
    /// LBB planning (request validation, assumption bindings, optional assignment extraction by stable ID).
    LogicBasedBendersPlan,
    /// 远端工件编解码（批次 A：`solver/remote` 的纯数据面，解码 + 完整性校验 + 规范化重编码 + 身份回显）。
    /// Remote artifact codec (batch A: the pure data surface of `solver/remote` — decode, integrity validation,
    /// canonical re-encode and identity echo).
    RemoteArtifactCodec,
    /// 冲突解释校验（批次 B：`core/analysis` 的 `ConflictExplanation::validate`）。
    /// Conflict-explanation verification (batch B: `ConflictExplanation::validate` from `core/analysis`).
    ConflictExplanationVerify,
    /// 候选漏斗校验与短名单（批次 B：`CandidateFunnelRanking::{validate,shortlist}`）。
    /// Candidate-funnel verification and shortlist (batch B: `CandidateFunnelRanking::{validate,shortlist}`).
    CandidateFunnelVerify,
    /// 有效性排序校验与投影（批次 B：`EffectivenessRanking::{validate,globally_effective,locally_effective}`）。
    /// Effectiveness ranking verification and projections (batch B).
    EffectivenessVerify,
    /// 关键约束总报告校验与阻塞摘要（批次 B：`CriticalConstraintAnalysisReport::{validate,has_minimal_blocking_set,blocking_summary}`）。
    /// Aggregate report verification and blocking summary (batch B).
    CriticalAnalysisVerify,
    /// 从已验证冲突生成带权松弛建议（批次 B：`correction::relaxation_recommendations_from_conflict`）。
    /// Weighted relaxation recommendations from a verified conflict (batch B).
    RelaxationRecommend,
    /// 固定整数 LP 灵敏度配置与报告（批次 B：`FixedIntegerLpSensitivityConfig::validate` + `LocalConstraintSensitivityReport`）。
    /// Fixed-integer LP sensitivity config and report (batch B).
    FixedIntegerLpVerify,
    /// RHS 扰动策略与报告（批次 B：`ConstraintPerturbationPolicy` 构造器/validate + `ConstraintPerturbationReport`）。
    /// RHS perturbation policy and report (batch B).
    PerturbationPolicyVerify,
    /// 目标构造与可行性报告（批次 B：`ObjectiveTarget` 六种构造器 + `exact_integer_bound` + `TargetFeasibilityReport`）。
    /// Target construction and feasibility report (batch B).
    TargetVerify,
    /// 多目标报告与跨目标 criticality（批次 B：`MultiTargetAnalysisReport::validate` 的跨目标重算核对）。
    /// Multi-target report and cross-target criticality (batch B).
    MultiTargetVerify,
    /// criticality profile 的独立构造与校验（批次 B：`build_criticality_profile` + `CriticalityProfile::validate`）。
    /// Standalone criticality profile build and verification (batch B).
    CriticalityProfileVerify,
    /// 超启发式配置的三格式往返与结构校验（批次 C：`ConfigurationFormats` + `HyperHeuristicConfig::validate`）。
    /// Hyper-heuristic config three-format round trip and validation (batch C).
    HyperHeuristicConfigVerify,
    /// 奖励策略构造器的纯校验与可恢复状态（批次 C：三个 `RewardPolicy::new` + `RecoverableComponent::save`）。
    /// Reward-policy constructor validation and restorable state (batch C).
    RewardPolicyVerify,
    /// 策略档案与完整检查点的结构校验 + Kotlin v1 导出能力审计（批次 C）。
    /// Policy-archive and run-checkpoint structure plus the Kotlin-v1 export audit (batch C).
    CheckpointVerify,
    /// 线性模型的稳定 solver 映射查询（批次 D：`linear_model_mapping` + `LinearModelMapping::validate`）。
    /// Stable solver mapping of the linear model (batch D).
    LinearMappingVerify,
    /// Logic-Based Benders 恢复身份预检（批次 D：`LogicBasedBendersResumeIdentity::validate`）。
    /// Logic-Based Benders resume-identity preflight (batch D).
    LbbResumeIdentityVerify,
    /// Constraint Programming 检查点的结构校验与类型化状态投影（批次 D：`ConstraintProgrammingCheckpointArtifact`）。
    /// Constraint Programming checkpoint structure and typed-state projection (batch D).
    CpCheckpointVerify,
    /// Benders 停止原因、运行时指标与逐轮快照的校验与投影（批次 E）。
    /// Benders stop reasons, runtime metrics and per-iteration snapshots (batch E).
    BendersMetricsVerify,
    /// Benders cut 的往返与结构报告（批次 E：`to_linear_cuts` / `to_quadratic_cuts` + 领域值反投影）。
    /// Benders cut round trip and structure report (batch E).
    BendersCutVerify,
    /// `symbol/functions` 的条件词汇与参数校验（批次 F：关系 / 真值 / 范围可判定性）。
    /// The condition vocabulary and parameter validation from `symbol/functions` (batch F).
    SymbolConditionVerify,
    /// big-M 标定旋钮与二值化方法的参数词汇（批次 F：`big_m` / `binaryzation`，不生成约束）。
    /// The big-M calibration knob and the binaryzation-method vocabulary (batch F).
    SymbolLinearizationVerify,
    /// `inequality` / `imply` / `if_in` 的即时展开核心关系表（批次 F：纯表读取，不生成约束）。
    /// The eager-expansion core relation tables of `inequality`, `imply` and `if_in` (batch F).
    SymbolCoreRelationsVerify,
    /// 启发式算子的参数调度曲线采样（批次 G：算子本身不绑，只采样参数演化）。
    /// Sampling the parameter schedules of heuristic operators (batch G).
    HeuristicScheduleSample,
    /// Q 表的编解码与确定性推演（批次 G：不建 agent、不接观测者、不跑搜索）。
    /// The Q-table codec and a deterministic replay (batch G).
    QTableSimulate,
    /// master cut 的编码分类与验证冲突子集（续挖：`logic_based_benders` 的纯数据面）。
    /// The master-cut encoding classification and verified conflict subset (continuation).
    MasterCutEncodingVerify,
    /// 跨阶段约束组汇总（续挖：`analysis/pipeline.rs` 的 `summarize_groups`）。
    /// The cross-stage constraint-group aggregation (continuation).
    AnalysisGroupAggregate,
    /// 谓词字段路径解析与 schema 映射（续挖：`persistence/expression`）。
    /// Predicate field-path resolution and schema mapping (continuation).
    PredicateFieldResolutionVerify,
    /// 谓词的三级字段过滤提取（续挖：`predicate_field_filter`）。
    /// The three-level predicate field-filter extraction (continuation).
    PredicateFieldFilterExtract,
    /// 模型构建阶段与模型配置面（续挖：`model/configuration` + `model/status`）。
    /// The model-building stages and the model configuration surface (continuation).
    ModelConfigurationVerify,
    /// 求值缓存与范围缓存的键身份及上下文语义（续挖：`model/value_cache` + `model/range_cache`）。
    /// The value- and range-cache key identity and context semantics (continuation).
    CacheKeyIdentityVerify,
    /// 平展缓存键的身份是地址而不是值（续挖：`model/flatten/cache_key`）。
    /// The flatten cache key's identity is an address, not a value (continuation).
    CacheKeyAddressVerify,
    /// 平展缓存的三条路径（续挖：`model/flatten/flatten_context` + `flattenable`）。
    /// The three paths through the flatten cache (continuation).
    FlattenCacheVerify,
    /// 更新赋值的三型与组合语义（续挖：`persistence/expression/update_assignment`）。
    /// The three update-assignment kinds and how they compose (continuation).
    UpdateAssignmentVerify,
    /// 布尔表达式作用域的收集与组合语义（续挖：`persistence/expression/boolean_expression_dsl`）。
    /// How the boolean-expression scope collects and combines (continuation).
    BooleanScopeVerify,
    /// 不支持谓词策略的三型与谓词注解（续挖：`persistence/expression/unsupported_predicate_policy`）。
    /// The three unsupported-predicate policies and the annotation surface (continuation).
    UnsupportedPredicateVerify,
    /// 表达式仓储两个默认方法的走向（续挖：`persistence/expression/repository_api`）。
    /// Where the expression repository's two default methods go (continuation).
    RepositoryCallVerify,
    /// 函数符号使用语境的四个派生判定（续挖：`model/intermediate/function_usage`）。
    /// The four derived predicates of the function-usage summary (continuation).
    FunctionUsageVerify,
    /// 输入域证明的区间代数与浮点指纹（续挖：`model/intermediate/deferred_function`）。
    /// The input-domain proof's interval algebra and the float fingerprint (continuation).
    InputDomainVerify,
    /// 原生写入指纹校验与"只可收紧"的域证明（续挖：`model/intermediate/deferred_function`）。
    /// The native-write fingerprint check and the tighten-only domain proof (continuation).
    NativeWriteVerify,
    /// 线性模型的三个纯变换（续挖：`model/intermediate/linear_triad_model` + `elastic`）。
    /// Three pure transforms of the linear model (continuation).
    ElasticModelVerify,
    /// 分析状态的合并是一个四元格（续挖：`analysis/pipeline::combine_status`）。
    /// Combining analysis statuses forms a four-element lattice (continuation).
    CombineStatusVerify,
    /// 数值文本的 Kotlin 兼容格式与其单向性（续挖：`heuristic/session/codecs`）。
    /// The Kotlin-compatible number text and how one-way it is (continuation).
    NumberTextVerify,
    /// Big-M 的两段式校验与三个指示约束构造器的落地形状（续挖：`symbol/functions/big_m`）。
    /// The two-stage Big-M check and the shape the indicator builders emit (continuation).
    BigMPolicyVerify,
    /// 条件的三值语义与 `if_` / `if_legacy` 两条入口的类型分叉（续挖：`symbol/functions/semantic`）。
    /// The three-valued condition semantics and the type split between the two entries (continuation).
    ConditionalSemanticsVerify,
    /// 评估预算的嵌套计费契约与随机流的可恢复性（续挖：`heuristic/session/budget`）。
    /// The nested accounting contract of the budget and the random stream's resumability (continuation).
    SearchBudgetVerify,
    /// 谓词 schema 的字段映射表与它两套解析口径的分歧（续挖：`persistence/expression/predicate_schema`）。
    /// The predicate schema's field table and the split between its two lookups (continuation).
    PredicateSchemaVerify,
    /// 排序描述符的组合代数与空值排序支持矩阵（续挖：`persistence/expression/sort_by`）。
    /// The sort descriptor's composition algebra and the null-ordering support matrix (continuation).
    SortByVerify,
    /// 变量类型的定义域注册表（续挖：`variable/variable_type`）。
    /// The variable-type domain registry (continuation).
    VariableKindDomainVerify,
    /// 临界约束分析报告的聚合规则（续挖：`analysis/pipeline` 的报告构建器）。
    /// The aggregation rules of the critical-constraint analysis report (continuation).
    CriticalAssemblyVerify,
    /// 展平缓存键的地址语义（续挖：`model/flatten` 的 `CacheKey` / `cache_key()`）。
    /// The address semantics of the flatten cache key (continuation).
    FlattenIdentityVerify,
    /// master cut 的两级不变量与表达式身份（续挖：`solver/logic_based_benders` 的 `MasterCut`）。
    /// The two-level invariants of a master cut and its expression identity (continuation).
    MasterCutVerify,
    /// 布尔表达式作用域的三条折叠规则（续挖：`persistence/expression/boolean_expression_dsl`）。
    /// The three folding rules of the boolean expression scope (continuation).
    BooleanFoldVerify,
    /// 字段解析的四态词汇与有损兼容桥（续挖：`persistence/expression/persistence_field_resolver`）。
    /// The four-state field-resolution vocabulary and its lossy compatibility bridge (continuation).
    FieldResolutionVerify,
    /// 标量函数词汇表是约定而不是约束（续挖：`persistence/expression/scalar_function_dsl`）。
    /// The scalar function vocabulary is a convention, not a constraint (continuation).
    ScalarFunctionVerify,
    /// 稀疏多项式三态的升幂契约（续挖：`model/flatten/inner_box` 的 `Linear` / `Quadratic` / `Canonical`）。
    /// The degree-lifting contract of the three sparse polynomial states (continuation).
    PolynomialLiftVerify,
    /// 有界整数 no-good 家族的行数与辅助变量契约（续挖：`solver/logic_based_benders`）。
    /// The row-count and auxiliary contract of a bounded-integer no-good family (continuation).
    BoundedNoGoodVerify,
    /// 有效性排序的聚合与两份改善判据（续挖：`analysis/pipeline` 的 `build_effectiveness_ranking`）。
    /// Effectiveness-ranking aggregation and its two improvement predicates (continuation).
    EffectivenessAggregateVerify,
    /// CP assumption 的稳定身份与主问题绑定的"恰好覆盖一次"契约（续挖：`solver/logic_based_benders`）。
    /// CP assumption stable identity and the "cover exactly once" master-binding contract (continuation).
    AssumptionBindingVerify,
    /// 候选分层的推导规则与"降权但不丢弃"的短名单（续挖：`analysis/pipeline` 的 `classify_candidate`）。
    /// How a candidate tier is derived, and the shortlist that deprioritises without dropping (continuation).
    TierClassificationVerify,
    /// 二进制 no-good 的极性编码与家族三行的系数/右端项闭式（续挖：`solver/logic_based_benders`）。
    /// The polarity encoding of a binary no-good plus the family rows' coefficient/rhs closed forms (continuation).
    PolarityEncodingVerify,
    /// 二次单项式的 kind 是推导出来的，且变量次序不被规范化（续挖：`model/flatten/inner_box`）。
    /// A quadratic monomial's kind is derived, and the variable order is not normalised (continuation).
    QuadraticMonomialVerify,
    /// 延迟函数物化的全有或全无与索引契约（续挖：`model/mechanism/basic_mechanism_model`）。
    /// The all-or-nothing materialization of deferred functions and its index contract (continuation).
    DeferredMaterializeVerify,
    /// 持久化后端的能力快照：族开关与方言开关是两个独立层（续挖：`persistence/backend`，真边界只绑纯配置）。
    /// The persistence backend capability snapshot: family and dialect flags are independent layers (continuation).
    BackendFeaturesVerify,
    /// 标准单项式的幂次表：幂次 0 与"变量不出现"读取上相同、表示上不同（续挖：`model/flatten/inner_box`）。
    /// The canonical monomial's power table: zero and absent read the same yet differ in representation (continuation).
    CanonicalMonomialVerify,
    /// `expressionKey()` 的位模式编码：它是位身份而不是数值身份（续挖：`solver/logic_based_benders`）。
    /// The bit-pattern encoding of `expressionKey()`: a bit identity rather than a numeric one (continuation).
    ExpressionKeyVerify,
    /// cut 的编码标签是元数据：`expressionKey()` 与 `normalize()` 都看不见它（续挖：`solver/logic_based_benders`）。
    /// A cut's encoding label is metadata: neither `expressionKey()` nor `normalize()` sees it (continuation).
    EncodingLabelVerify,
    /// 活动性判定的四条容差带，以及相对容差对零尺度的抬升（续挖：`analysis/activity`）。
    /// The four tolerance bands of activity classification, and how the relative tolerance lifts zero scale (continuation).
    ActivityBandsVerify,
    /// 活动性证据的两个谓词：`satisfied()` 的 `None` 把三件事混成一个（续挖：`analysis/activity`）。
    /// The two predicates on activity evidence: `satisfied()`'s `None` conflates three things (continuation).
    EvidencePredicatesVerify,
    /// 活动性报告的 `validate()`：聚合严查、数值只看有限性（续挖：`analysis/activity`）。
    /// `validate()` of the activity report: strict about aggregates, only finiteness for numbers (continuation).
    ActivityReportCheckVerify,
    /// 辅助变量的整数值域校验：`|界| > 2^53` 拒、恰好 `2^53` 放行（续挖：`solver/logic_based_benders`）。
    /// The integer-domain validation of auxiliaries: `|bound| > 2^53` is refused while exactly `2^53` passes (continuation).
    AuxiliaryDomainVerify,
    /// `MasterBinding` 与 `MasterAssignment` 的成对契约：恰好覆盖、域按 `0..=1` 与 `lower..=upper`
    /// （续挖：`solver/logic_based_benders`）。
    /// The paired contract of `MasterBinding` and `MasterAssignment`: exact coverage with `0..=1` and `lower..=upper`
    /// domains (continuation).
    BindingAssignmentVerify,
    /// `DiagnosticSource` 的六种来源与身份合并：`stable_id` 不能单独区分种类（续挖：`analysis/mod`）。
    /// The six `DiagnosticSource` kinds and where identities merge: a `stable_id` alone cannot tell the kind (continuation).
    SourceIdentityVerify,
    /// `MasterCutValidity` 与 `MasterCutProofStatus`：`Local` 从不被拒、两者都不进 `expression_key`
    /// （续挖：`solver/logic_based_benders`）。
    /// `MasterCutValidity` and `MasterCutProofStatus`: `Local` is never refused and neither enters the `expression_key`
    /// (continuation).
    CutValidityVerify,
    /// 更新赋值集合的组合语义与属性路径的三种校验强度（续挖：`persistence/expression`）。
    /// The combinator semantics of update assignments and the three validation strengths of a property path (continuation).
    AssignmentCompositionVerify,
    /// 中间模型转储选项：两个边界开关互相独立、固定变量表可被整表替换（续挖：`model/intermediate`）。
    /// The intermediate-model dump options: the two bound switches are independent and the fixed-variable map can be replaced
    /// wholesale (continuation).
    DumpOptionsVerify,
    /// 原生写入记录与 lowering 报告：两个谓词互补、两个计数不与结果交叉校验（续挖：`model/intermediate`）。
    /// Native write records and the lowering report: two predicates are complements while two counters are never cross-checked
    /// against the outcomes (continuation).
    NativeWriteScopeVerify,
    /// 原生写入器注册表的调度契约：先注册先决定、跳过不等于回退（续挖：`model/intermediate`）。
    /// The native writer registry's scheduling contract: first registered decides first, and a skip is not a fallback
    /// (continuation).
    WriterRegistryVerify,
    /// 候选漏斗的昂贵分析短名单：只优先 A/B、低层填剩余名额、且从不按 priority 排序（续挖：`analysis/pipeline`）。
    /// The candidate funnel's shortlist: only A/B are preferred, lower tiers fill the remaining slots, and it never sorts by
    /// priority (continuation).
    CandidateShortlistVerify,
    /// 惰性平展上下文的初始化语义：`init` 先到先得、未初始化读 `token_list()` 会 panic
    /// （续挖：`model/flatten`）。
    /// The lazy flatten context's initialization semantics: `init` is first-wins and reading `token_list()` before init panics
    /// (continuation).
    LazyFlattenInitVerify,
    /// 批量导出的分派与并发语义：`concurrent` 只改调度、`OPM` 忽略转储选项、失败不回滚
    /// （续挖：`model/intermediate`）。
    /// Batch export's dispatch and concurrency semantics: `concurrent` only changes scheduling, `OPM` ignores the dump
    /// options, and a failure rolls nothing back (continuation).
    DumpBatchVerify,
    /// 二次弹性构建器的开关语义：只弹性化线性行、关掉后连目标方向都不动
    /// （续挖：`model/intermediate`）。
    /// The quadratic elastic builder's switch semantics: only linear rows are elasticized, and disabling it leaves even the
    /// objective direction alone (continuation).
    QuadraticElasticVerify,
    /// 稳定 ID 取值：先 `validate()`、缺解与缺 ID 共用一条消息、且完全不看绑定
    /// （续挖：`solver/logic_based_benders`）。
    /// Stable-ID lookup: `validate()` first, one message for both failures, and the binding is never consulted
    /// (continuation).
    StableValueVerify,
    /// master 赋值提取的容差与范围语义：只收最优报告、`round()` 半数远离零、两道范围闸
    /// （续挖：`solver/logic_based_benders`）。
    /// The master-assignment extraction's tolerance and range semantics: optimal reports only, `round()` half away from zero,
    /// and two range gates (continuation).
    MasterAssignmentVerify,
    /// 稀疏三角模型的手写语义：`add` 从不合并、重复 ID 只覆盖索引、`setBounds` 不校验空域
    /// （续挖：`model/intermediate`）。
    /// The sparse triad model's hand-written semantics: `add` never merges, a duplicate ID only overwrites the index, and
    /// `setBounds` accepts an empty domain (continuation).
    SparseTriadVerify,
    /// 约束行与元数据的锁步语义：`numConstraints` 只读 `b`、八个平行向量同进同退
    /// （续挖：`model/intermediate`）。
    /// The lockstep semantics of rows and metadata: `numConstraints` reads only `b`, and the eight parallel vectors move
    /// together (continuation).
    MatrixRowsVerify,
    /// 线性/二次多项式的结构变更语义：`addMonomial` 纯追加、`zero`/`constant` 同形
    /// （续挖：`model/flatten`）。
    /// The structural mutation semantics of the linear and quadratic polynomials: `addMonomial` is pure append, and
    /// `zero`/`constant` share a shape (continuation).
    PolynomialProbeVerify,
    /// 两个只读视图 trait 的口径：名字取自线性部分、`numConstraints` 只数线性行、目标向量不对账
    /// （续挖：`model/intermediate`）。
    /// The two read-only view traits' conventions: the name comes from the linear part, `numConstraints` counts linear rows
    /// only, and the objective is unreconciled (continuation).
    ModelViewVerify,
    /// 分析流水线阶段配置的四道闸门：flag 不校验、`candidateLimit` 是快照、`validate()` 按阶段顺序短路
    /// （续挖：`analysis/pipeline.rs`）。
    /// The four gates of the analysis pipeline's stage configuration: the flags are never validated, `candidateLimit` is a
    /// snapshot, and `validate()` short-circuits in stage order (continuation).
    AnalysisOptionsVerify,
    /// 能力矩阵的推断格：空矩阵也敢说支持活动性、覆盖项只改查询不改"原生"、`Conditional` 也算可用
    /// （续挖：`analysis/mod.rs` 的 `inferred_support`）。
    /// The capability matrix's inference lattice: an empty matrix still claims activity support, an override changes the
    /// query but never the native flag, and `Conditional` counts as available (continuation).
    CapabilityMatrixVerify,
    /// CP 支持报告合并进能力矩阵的推导口径：原生标志只认 descriptor、`exactCpLowering` 空真、
    /// 逐约束等级不进矩阵（续挖：`analysis/mod.rs` 的 `from_constraint_programming_support`）。
    /// The derivation rules that merge a CP support report into the matrix: native flags come from the descriptor only,
    /// `exactCpLowering` is vacuously true, and per-constraint levels never enter the matrix (continuation).
    CpSupportMergeVerify,
    /// CP subproblem 四态结果的构造闸门：`failed` 从不校验、只有 snapshot 入口发证明、`incomplete` 连最优都拒
    /// （续挖：`solver/logic_based_benders.rs`）。
    /// The construction gates of the CP subproblem's four-state result: `failed` never validates, only the snapshot
    /// constructors issue proofs, and `incomplete` refuses even an optimal report (continuation).
    SubproblemResultVerify,
    /// 默认 feasibility no-good oracle 真正生成出来的 cuts：二进制域一条、有界整数域 `3n+1` 条、空 conflict
    /// 退化为整份赋值、context 入口不校验引用（续挖：`solver/logic_based_benders.rs`）。
    /// The cuts the default feasibility no-good oracle actually emits: one for binary domains, `3n+1` for bounded
    /// integers, an empty conflict degrading to the whole assignment, and a context entry that skips provenance.
    NoGoodOracleVerify,
    /// CP 模型 builder 的提交闸门与指纹口径：`new` 自带身份、两道去重、`setObjective` 静默覆盖、
    /// `freeze` 重绑本地 ID 且模型名不进指纹（续挖：`model/constraint_programming/model.rs`）。
    /// The commit gates of the CP model builder and what its fingerprint covers.
    CpModelBuildVerify,
    /// CP 快照自己的读侧闸门：`validateIdentity` 的七道检查、`validateHint` 的部分赋值口径、
    /// `objectiveVariables` 的引用范围、artifact codec 的三道关（续挖：`model/constraint_programming/snapshot.rs`）。
    /// The CP snapshot's own read-side gates.
    SnapshotGateVerify,
    /// CP 约束 AST 的规范化：`canonicalize` 只排序"顺序无意义"的变体，从不合并也从不化简
    /// （续挖：`model/constraint_programming/constraint.rs`）。
    /// The normalization of the CP constraint AST: `canonicalize` sorts only the variants whose order is irrelevant and
    /// never merges nor simplifies.
    ConstraintCanonVerify,
    /// `validateAssignment` 的求值顺序：先覆盖、再域、再区间、最后按规范序逐约束
    /// （续挖：`model/constraint_programming/snapshot.rs` 的 `validate_assignment`）。
    /// The evaluation order inside `validateAssignment`: coverage, domains, intervals, then constraints in canonical order.
    AssignmentEvalVerify,
    /// `IntegerExpression::bounds` 的保守界：只读域的包络，不看稀疏空洞
    /// （续挖：`model/constraint_programming/expression.rs`）。
    /// The conservative bounds of `IntegerExpression::bounds`: it reads only the domain hull, never the sparse gaps.
    ExpressionBoundsVerify,
    /// `IntegerDomain` 的取值面：基数、枚举闸门、成员判定与规范字节
    /// （续挖：`model/constraint_programming/domain.rs`）。
    /// The value surface of `IntegerDomain`: cardinality, the enumeration gate, membership and the canonical bytes.
    DomainSurfaceVerify,
    /// LBB 的身份包装器：指纹闸门只查"非空白"，三条构造器两种文案
    /// （续挖：`solver/logic_based_benders.rs` 的 `IdentifiedMaster` / `IdentifiedSubproblemSolver` /
    /// `SnapshotSubproblemFactory`）。
    /// The LBB identity wrappers: the fingerprint gate only checks "not blank" and three constructors share two messages.
    IdentityWrapVerify,
    /// 展平层的容器面："空"不等于"零"，且改值不改缓存键
    /// （续挖：`model/flatten/inner_box.rs` 的 `Linear` / `Quadratic` / `Canonical`）。
    /// The flatten containers: "empty" does not mean "zero", and mutating the value keeps the cache key.
    ContainerSurfaceVerify,
    /// 诊断激活集合的语义：缺席即未激活、`insert` 覆盖并移位、两个变体共享一个稳定身份
    /// （续挖：`analysis/activation.rs`）。
    /// The diagnostic activation set: absence means inactive, `insert` overrides and re-orders, and two variants share
    /// one stable identity.
    ActivationSetVerify,
    /// 冲突提取的层级选择：`select` 走 `support()` 而 `is_native` 绕过覆盖，所以"选中原生层"与
    /// "原生标志为真"是两件事（续挖：`analysis/activation.rs` 的 `ConflictExtractionTier`）。
    /// The conflict-extraction tier selection: `select` goes through `support()` while `is_native` bypasses overrides.
    TierSelectionVerify,
    /// 后端证据的回映：直接成员优先、assumption 路径靠共享身份命中、`ObjectiveTarget` 与 `Interval`
    /// 永远匹配不上（续挖：`analysis/activation.rs` 的 `remap_backend_evidence` / `source_of_assumption`）。
    /// The remapping of backend evidence: direct members first, the assumption path hitting through a shared identity,
    /// and two kinds that can never match.
    EvidenceRemapVerify,
    /// 支配关系：判据只看方向表长度、`NaN` 被当作"相等"、方向表短了会截断（续挖：启发式层的
    /// `multi_objective/dominance.rs`）。
    /// The dominance relation: the predicate reads only the direction-list length, `NaN` counts as equal, and a short
    /// direction list truncates.
    DominanceVerify,
    /// 非支配排序与拥挤距离：分层只按输入顺序、重复点同层、方向表截断会重排分层，而拥挤距离的边界解在
    /// 常量轴上也是无穷、且只按第一个点的维度数走（续挖：启发式层的 `non_dominated_sort.rs`）。
    /// Non-dominated sorting and crowding distance: the layering keeps the input order, duplicates share a front, a
    /// truncated direction list re-layers, the crowding boundary is infinite even on a constant axis, and the axis count
    /// comes from the first point.
    DispersionVerify,
    /// NSGA-III 参考点：点的个数是单纯形格点、顺序按首坐标字典序、一个目标必塌成单位点，而垂直距离忽略
    /// 参考方向之外的维度且对参考方向尺度不变（续挖：启发式层的 `reference_points.rs`）。
    /// The NSGA-III reference points: the count is the simplex lattice, the order is lexicographic in the first
    /// coordinate, one objective collapses to the unit point, while the perpendicular distance ignores dimensions past
    /// the reference and is scale-invariant in the reference direction.
    ReferencePointsVerify,
    /// 有界 Pareto 档案：被支配的丢、支配的淘汰、重复目标向量先到者胜，而裁剪按拥挤距离重排后截断；
    /// 整批只在末尾裁一次而逐个每插一个就裁，两者会分歧（续挖：`pareto_archive.rs`）。
    /// The bounded Pareto archive: dominated candidates are dropped, dominating ones evict, duplicates keep the first
    /// offer, and the trim reorders by crowding distance; a batch trims once at the end while singles trim after every
    /// insert, so the two can disagree.
    ParetoArchiveVerify,
    /// 约束评估的两种排序：epsilon 排序在容差内让目标说了算（不可行的能排到可行之前），随机排序用调用方
    /// 种子可复现、可行对必按目标比（续挖：启发式层的 `fitness/constraint_ranking.rs`）。
    /// The two rankings over constraint assessments: inside the tolerance the epsilon ranker lets the objective decide,
    /// while the stochastic ranker is reproducible from a caller-owned seed and always compares feasible pairs by
    /// objective.
    ConstraintRankingVerify,
    /// 约束优先级的双层身份：枚举身份与存储值是两层，`value()` 不是单射——`Custom(0/10/50/100)` 与四个
    /// 有名变体共享存储值，而 `From<u32>` 只认那一个规范代表（续挖：`model/basic/constraint_priority.rs`）。
    /// The two-layer identity of a constraint priority: `value()` is not injective, and `From<u32>` accepts only the one
    /// canonical representative.
    ConstraintPriorityVerify,
    /// 种群配置的构造面：构造器给 `0.1 / 1`、`mutation_rate` 一字不校验、`archive_amount` 的 `.max(1)`
    /// 只活在 builder 里（字段全 `pub`，字面量直接绕过它）。只绑纯配置，算子槽位用 crate 自己的默认组合。
    /// The configuration surface of population search: the constructor defaults to `0.1 / 1`, the mutation-rate builder
    /// validates nothing, and the archive `.max(1)` lives only in the builder.
    PopulationConfigVerify,
    /// 三种边界处理的退化路径：`NaN` 三路都落到底、`+∞` 让截断分道扬镳、倒置区间只有截断 panic、
    /// 反射折 64 期就放弃、环绕把区间当半开区间（续挖：`heuristic/operator/boundary.rs`）。
    /// The degenerate paths of the three boundary handlers.
    BoundaryVerify,
    /// 接受判据的决策面：`AlwaysAcceptance` 只比约束层级（于是并非"无条件"）、`BetterOnly` 与
    /// `NotWorse` 的差别只在平局、`Metropolis` 先按层级结案再抽 `exp(-Δ/T)`
    /// （续挖：`heuristic/operator/acceptance.rs`）。
    /// The decision surface of the acceptance criteria.
    AcceptanceVerify,
    /// 三个排序算子：`ObjectiveRanking` 只是把模型的 `rank_indices` 抄回来（所以"升序"是模型说了算），
    /// `ReversedObjectiveRanking` 先升序再整体 `reverse()`（于是并列块被倒过来）（续挖：`operator/ranking.rs`）。
    /// The three ranking operators.
    RankingProbeVerify,
    /// 五种邻域结构的下标算术：空种群仍报邻居 0、普通环把越界下标原样留下、k 邻环绕回、
    /// 两成员的环退化成完全图、网格不在边界回卷且末行不满时会丢邻居
    /// （续挖：`heuristic/operator/topology.rs`）。
    /// The index arithmetic of the five neighbourhood structures.
    TopologyVerify,
    /// 两种岛屿迁移的空岛与并列语义：环形迁移的 `.max(1)` 让迁移率 0 也照迁一个、空岛只让后继拿到 0 个；
    /// 优岛到劣岛则任一岛为空就整体放弃、并列时一个都不动、且**目标岛可能被撑大**
    /// （续挖：`heuristic/operator/migration.rs`）。
    /// The empty-island and tie semantics of the two island migrations.
    MigrationVerify,
    /// 两条单目标档案：`BestArchive` 的"是否改进"旗标说的是**档案最优**而不是这批候选（首轮必真、
    /// 容量 0 永远假、等值候选被存下却不报改进），`HistoryBestArchive` 只要这批里有一个更优就报真且不收等值
    /// （续挖：`heuristic/operator/archive.rs`）。
    /// The two single-objective archives.
    ArchiveProbeVerify,
    /// 分段线性构造器的闸门：两点底线、x 必须严格递增、坐标有限、结果值域是 y 的包络；
    /// 而 `from_segments` **不校验线段之间的连续性**，每个断点取**右边那条**线段的值，不一致的图被静默改写
    /// （续挖：`symbol/functions/univariate_linear_piecewise.rs`）。
    /// The gates of the piecewise-linear constructor.
    PiecewiseVerify,
    /// 两个多样性度量对"维度"与"缺失值"的回答不一致：均值度量的维度取**第一个解的条目数**、缺值传播成 `NaN`；
    /// 分形度量看 `order` 的每个位置、缺值补 0；两者都把退化种群读成 0
    /// （续挖：`heuristic/operator/diversity.rs`）。
    /// The two diversity metrics disagree about dimension and missing values.
    DiversityVerify,
    /// 带状态解的六个状态与两条判据：默认 `NotSolved`、`with_status` 丢目标值、`is_feasible` 只认
    /// `Optimal | Feasible`（于是 `Unbounded`/`Unknown` 不算可行）、`with_objective` 不校验状态
    /// （续挖：`model/callback/solution.rs`）。
    /// The six statuses of the status-carrying solution and its two predicates.
    SolutionStatusVerify,
    /// 候选漏斗的分层全在"活动性 + 对偶"的组合里：`NearlyActive` 与 `Active` 同档、**`NaN` 对偶落进 Tier B**、
    /// active 但无对偶只能 `Unclassified`、`Inactive` 带巨大对偶仍是 Tier C、`Violated` 落到 `Unclassified`；
    /// `as_str()` 与线上名不一致；分类器不校验阈值、配置不校验上限
    /// （续挖：`analysis/pipeline.rs` 的 `classify_candidate` 与 `CandidateFunnelRanking::shortlist`）。
    /// The candidate funnel's tiering and shortlist.
    FunnelVerify,
    /// 有效性排序的"排序"从不被校验：`validate()` 只看身份与两条前提、**不看顺序**；`max_improvement` 要严格正才算
    /// 全局有效（0 与负数都不报错）；局部有效只认 `Some(true)`；`first_effective_delta` 必须为正；
    /// `unavailable_reason` 与 `status` 之间没有任何约束（续挖：`analysis/pipeline.rs` 的 `EffectivenessRanking`）。
    /// The effectiveness ranking: ordering unvalidated, positive-only global effectiveness, tied-to-nothing reason.
    EffectivenessProbeVerify,
    /// 统一分析报告的 `validate()` **不检查总体结论**：没有阶段也能带"已证明"通过；它查的是字符串——
    /// 用 `Debug` 格式化整份报告再扫描禁用词（于是调用方自己的理由里带 `solver_row` 也被拒）；
    /// 没有冲突解释时 `has_minimal_blocking_set()` 为假、摘要逐字是"未执行"
    /// （续挖：`analysis/pipeline.rs` 的 `CriticalConstraintAnalysisReport`）。
    /// The unified analysis report: the overall conclusion is never validated; the forbidden-word scan is textual.
    CriticalReportVerify,
    /// 跨阶段组汇总的四条口径：顺序是阻塞降序 → active 降序 → 组名升序（未分组只在并列时先行）；
    /// 组由活动性每条记录 / 有效性**带改善**的条目 / 冲突的阻塞身份落地；`max_improvement` 取最大值；
    /// `active_instances` 只数 `Active`（`NearlyActive` 不算）（续挖：`analysis/pipeline.rs` 的 `summarize_groups`）。
    /// The cross-stage group summary and its four readings.
    GroupSummaryVerify,
    /// 活动性分档的边界语义：恰好 `-active` 落在 `Active` 档（`slack < -active` 是严格不等）、
    /// 容差随尺度**只增不减**、nearly 档被 `.max(active)` 抬高（非法配置也能分类但 `validate()` 拒）、
    /// 非有限 slack 或尺度给 `Unknown`；报告的两个计数必须等于重算结果
    /// （续挖：`analysis/activity.rs` 的 `ActivityConfig` / `ConstraintActivityReport`）。
    /// The activity banding edges and the report's counting invariants.
    BandProbeVerify,
    /// 不可行证据的权威性闸门：`NativeIis`/`Farkas` 无条件权威，CP 重建必须同时 `EXACT` 与 `COMPLETE`，
    /// 两种过滤与 `Unavailable` 永不权威；`minimality` 默认 `NOT_CHECKED`；`constraint_ids` 与 `members`
    /// 是两套互不推导的集合；`unavailable_reason` 与状态不联动（续挖：`solver/report/mod.rs` 的 `InfeasibilityEvidence`）。
    /// The authority gate for infeasibility evidence.
    EvidenceProbeVerify,
    /// 统一报告的证明合同与精确定价闸门：已核验的证明必须同时完整、可靠且以 `Completed` 收尾；
    /// `Optimal` 的 incumbent 要求类型正确、`Verified`、可靠、完整的最优性证明；`None` 状态被拒；
    /// 闸门在报告合法之后还要一个非空对偶解（续挖：`solver/report/mod.rs` 的 `SolveReport::validate`）。
    /// The report's proof contracts and the exact-pricing gate.
    ProofProbeVerify,
    /// 报告里的算术自洽：绝对 gap 必须等于 incumbent 与下界之差、**缺任一侧时 gap 整体不许出现**、
    /// 相对 gap 的分母以 1 为地板；负与非有限 gap 被拒；逐轮快照要求编号从 1 起连续、阶段名非空、
    /// 累计条目数不下降、每轮 gap 与自身数字自洽（续挖：`solver/report/mod.rs` 的 gap 与 trace 两段）。
    /// The report's gap arithmetic and the iteration-trace invariants.
    GapProbeVerify,
    /// 诊断面与读侧谓词的边界：`has_incumbent()`/`is_optimal()` **只读 `solution_presence`**、**从不校验报告**，
    /// `incumbent()` 读的是 `solution.value`；而**诊断面完全不受 `validate()` 约束**——荒谬的求值记录、空白
    /// warning 与 issue、`satisfied` 与 `violation` 自相矛盾全都通过（续挖：`solver/report/mod.rs`）。
    /// The diagnostic face and the read-side predicate boundaries.
    DiagnosticsProbeVerify,
    /// 指纹与 provenance 的边界：`SolverProvenance::default()` 用 `unknown` 命名；`SolveFingerprints` 默认三个 `None`；
    /// 指纹相等是**三个字段的结构相等**（同值不同算法不相等），而这个比较是证书闸门里唯一真正读指纹的地方，
    /// 报告没带模型指纹时闸门直接拒；provenance 与两处 schema 版本**从不被校验**
    /// （续挖：`solver/report/mod.rs` 的 `SolverProvenance` / `SolveFingerprints` / `AuditFingerprint`）。
    /// The fingerprint and provenance boundaries.
    FingerprintProbeVerify,
    /// 最小阻塞集合的最小性判据：`verified()` **只读 `minimality == Irreducible`**，状态、有效性、复验记录、
    /// 成员、背景阻塞、提取层级全都不参与；`is_verified()`/`as_solver_minimality()` 是同一套三分（默认
    /// `NotChecked`）；`is_native()` 只认 `NativeUnsatCore`（续挖：`analysis/conflict.rs` 的 `MinimalBlockingSet`）。
    /// The minimality verdict of the minimal blocking set.
    BlockingSetVerify,
    /// 修正建议的排序与截断：按 `weight × relaxation` **升序**（并列按稳定 ID）、**截断留下最便宜的**、
    /// `total_cost` 是乘积之和且必须自洽、带权集合**从不 minimal**、`minimal` 入口**必须声明已复验**且
    /// **一旦截断就退回 `minimal = false`**；`is_complete_mcs()` 恒假、`requires_revalidation()` 恒真
    /// （续挖：`analysis/correction.rs` 的 `weighted_correction_set` / `minimal_correction_set`）。
    /// The ordering and truncation of the correction recommendation.
    CorrectionProbeVerify,
    /// 定点整数 LP 灵敏度的口径：两个容差只要求有限且非负、**没有顺序约束**；`has_duals()` 只问"有没有 `Some`"
    /// （零对偶算、非有限对偶也算，而 `validate()` 拒后者）；按身份查记录**先到先得**；范围名与线上名不一致
    /// （续挖：`analysis/fixed_integer.rs` 的 `FixedIntegerLpSensitivityConfig` / `LocalConstraintSensitivityReport`）。
    /// The fixed-integer LP sensitivity readings.
    SensitivityProbeVerify,
    /// 候选漏斗的分层、排序与短名单：A/B 分界是 `|dual| > threshold`（**严格大于**）、`Active` 与
    /// `NearlyActive` 同权、只有 `Inactive`/`SatisfiedWithoutSlackMetric` 到 C、**无灵敏度报告即无分层**、
    /// 排序是层级→|dual| 降序→身份升序、短名单先放 A/B、`local_effective` 由漏斗自己重算、配置的
    /// `default_candidate_limit` **在漏斗里不生效**（续挖：`analysis/pipeline.rs` 的 `build_candidate_funnel`）。
    /// The candidate funnel's tiers, ordering and shortlist.
    FunnelRankingVerify,
    /// 有效性排序的两个视角与校验口径：全局有效只看 `max_improvement > 0`、两个视角都**按原顺序过滤**且
    /// `validate()` **从不检查排序**、局部有效只认恰好 `Some(true)`、两个视角**互不蕴含**、校验拒重复身份与
    /// 非有限数值、`first_effective_delta` **必须为正**、`local_effective` **必须有对偶**、`removal_effective`
    /// **只在 `Unsupported` 时被拒**（续挖：`analysis/pipeline.rs` 的 `EffectivenessRanking`）。
    /// The effectiveness ranking's two views and its validation readings.
    EffectivenessRankingVerify,
    /// 持久化排序描述的构建与支持判据：`then` **纯追加**且**不合并同路径**、`Add` 就是 `then`、空/非空互补、
    /// `is_supported` **只问方向**且**不看 `nulls` 字段**（`Auto`/`Always` 恒真、`Never` 恒假、`OnlyAsc` 只认
    /// `Asc`）、`*_nulls` 与普通构造只差 `nulls`、`*_many` 保留输入顺序
    /// （续挖：`persistence/expression/sort_by.rs` 的 `SortBy`）。
    /// The persistence sort descriptor's building and support readings.
    PersistenceSortVerify,
    /// 谓词 schema 的两个查表面**互相矛盾**：`resolve()` 用 `find` → **先到先得**，`to_field_map()` 用 `HashMap`
    /// 收集 → **后到覆盖**（同路径写两条时两个 API 给出相反答案）；`with_same_field()` 把**整条路径文本**当后端名；
    /// 未登记路径一律 `None`；`field()` **不校验登记**；映射表按路径去重
    /// （续挖：`persistence/expression/predicate_schema.rs` 的 `PredicateSchema`）。
    /// The predicate schema's two contradictory lookup faces.
    PersistenceSchemaVerify,
    /// 谓词收集与不支持策略：空作用域 `build_and()` **恒真**、`build_or()` **恒假**、**单项直接返回该项**、
    /// 两项以上才真的合并；builder 返回表达式本身并把**副本**收进作用域（`build_*` 消费 `self`）；`Default` 即
    /// `new()`；`UnsupportedPredicatePolicy` 里 **`FailFast` 是唯一 fail-fast、`AlwaysFalse` 是唯一恒假，
    /// `ClientFilter` 两者皆假**，默认 `AlwaysFalse`，`IGNORE`/`ERROR` 只是别名
    /// （续挖：`persistence/expression/boolean_expression_dsl.rs` 与 `unsupported_predicate_policy.rs`）。
    /// Predicate collection and the unsupported predicate policy.
    PredicateScopeVerify,
    /// native 终态证明门：四条臂各自要求（native 状态、problem 状态、**incumbent 存在性**、`Completed`）同时成立
    /// 才写证明；**不匹配组合走 `_ => {}`**，既不写证明也**不直接报错**（裁判权在结尾的 `report.validate()`）；
    /// 重写证明时**旧证据被搬过去**；`Optimal` 臂还把 presence 设为 `Optimal`
    /// （续挖：`solver/report` 的 `verify_native_terminal_proof`）。
    /// The native terminal proof gate.
    NativeTerminalProofVerify,
    /// 原生 lowering 报告的三个判定各看各的字段：`is_fully_native()` 要求 `outcomes` **非空**且
    /// `materialized_fallbacks == 0`，`has_fallbacks()` 只看计数 > 0，于是**空报告既不全原生也无退化**、两者
    /// **不互补**；两者都**不数 `outcomes`**，所以计数撒谎的报告仍然通过；报告**不校验**
    /// `native_writes + materialized_fallbacks == outcomes.len()`；`is_native()`/`requires_fallback()` 严格互补；
    /// `FallbackReason` 的相等是结构相等（`Rejected("x") != WriterFailed("x")`）
    /// （续挖：`model/intermediate/native_writer.rs`）。
    /// The self-check gaps of the native lowering report.
    NativeLoweringVerify,
    /// 求解值的数值域转换：`SolveValue for f64` 的两个方向**只看 `is_finite`**，于是 `Strict` 与 `AllowRounding`
    /// 对 `f64` 完全等价；默认策略是 `Strict`；`to_nonnegative_integral_upper_bound` 的默认实现把策略写死为
    /// `AllowRounding`、负数报 "cannot be negative"、`>= 2^63` 报溢出、其余**取 `floor`**（`-0.0` 不算负数）
    /// （续挖：`solver/value/solve_value.rs` 的 `SolveValue`）。
    /// The solve value's numeric-domain conversion and the floored upper bound.
    SolveValueVerify,
    /// 超启发式选择策略：七种 kind 只分五个臂（`fixed` 与 `round-robin` 同一条臂，游标推进成"选中索引 + 1"）；
    /// `stagnation` 只在**非零且为 limit 整数倍**时推进；`ucb1` 学习态**先选未观察动作**；`learning == false`
    /// 时 `observe` **什么都不学**；只有 `q-learning`/`epsilon-greedy`/`ucb1` 累计；`snapshot().table` **只有
    /// `q-learning` 非空**；六类非法定义一律被拒
    /// （续挖：`solver/hyper_heuristic/selection.rs`）。
    /// The hyper-heuristic selection policy and its five arms.
    SelectionPolicyVerify,
    /// 非平稳选择器：`frr-mab` 的引导只挑**掩码内未观察**的动作且按**动作规范顺序**；窗口滑过 `window` 丢最旧；
    /// 归一让奖励**尺度无关**（负奖励照进窗口、权重按 0 计）；`choice-function` 在 `select` 时**冻结前驱**，
    /// 同一决策**重复派发直接报错**（`discard` 才放开），`terminal` 完成**清空前驱**；`adaptive-pursuit` 的
    /// `rate == 1` **直接落到目标**；`learning == false` 时**只校验不写**
    /// （续挖：`solver/hyper_heuristic/selection/non_stationary.rs`）。
    /// The non-stationary selector's credit assignment.
    NonStationaryVerify,
    /// 训练切分的校验口径：**ID 非空且跨三个集合全局唯一**、**训练集非空**（验证/测试为空允许）、
    /// **同一问题不得跨集合出现**（同一集合内允许重复）；`problem` 按 `ComponentSpec` 的**结构相等**比较，
    /// 只差 `version` 或只差一个参数就算不同问题；同一 id 又跨集合时**先报 ID 那条**
    /// （续挖：`solver/hyper_heuristic/learning.rs` 的 `TrainingSplit`）。
    /// The training split's validation contract.
    TrainingSplitVerify,
    /// 三种接受准则的定义锁与历史锁：`late` 的 `length` 只在 `(0, i32::MAX]` 内、`load` 拒改过的定义与
    /// 不自洽历史；`record-to-record` 的 ratio 只在 `[0,1]` 内、`load` 拒 `|best| > scale` 与时钟不同步；
    /// `auto` 的 `capacity = sqrt(horizon).clamp(8,256)`、`span = (capacity/2).max(2)` 是推导的，
    /// `load` 拒 `span` 越界、`improvements` 超额与历史长度不符（续挖：`memory/adaptive.rs`）。
    /// The three acceptance policies' definition and history locks.
    AcceptanceRecoveryVerify,
    /// 超体积增量的三段式生命周期：校准阶段 `delta = 0` 且 **`calibrated = false`**；凑满校准的那一次**冻结
    /// 自动参考点**并仍 `delta = 0`；此后 `exact` 的增量**非负且带 `work`**，`sampled` 给 `exact = false` 与
    /// 标准误/99% Hoeffding 界；**空候选（最小化下更差）给 0**；维度不一致与非有限坐标**直接拒**；
    /// `maxWork = 0`/`samples = 0` 构造即拒；**失败不覆盖上一次成功**（续挖：`reward.rs`）。
    /// The hypervolume increment's three-phase lifecycle.
    HypervolumeVerify,
    /// 算子能力声明的三道门：**空 `encodings` 被拒**、**非相邻重复也被拒**（判重读整个前缀）、
    /// **`stateless` 不参与校验**、**空 id 与重复 id 被拒**、**能力校验先于 id 校验**、
    /// **update/mutation/neighborhood 三个面各管各的能力表**（续挖：`operator_registry.rs`）。
    /// The operator capability declaration's three gates.
    OperatorCapabilityVerify,
    /// 调度日志的身份门与九条顺序规则：**校验和是先决条件**（不先按框架自己的规范形式算出摘要，
    /// 这条门之后的状态机永远摸不到），随后**`begin` 不可重复**、**`dispatch` 四个前置**、
    /// **`complete` 只准缩不许长**、**`drain`/`yield` 要有状态**、**`end` 要静止且总数对得上**、
    /// **`end` 之后不许再有事件**、**事件默认值不上线材**（续挖：`schedule.rs`）。
    /// The schedule log's identity gate and its nine ordering rules.
    ScheduleLogVerify,
    /// 谓词字段过滤提取（`predicate_field_filter`）：能分解就拆成字段条件，不能就如实报不可分解。
    /// Predicate field-filter extraction (`predicate_field_filter`): decompose into field conditions when possible,
    /// otherwise report "not decomposable" faithfully.
    PredicateFieldFilters,
    /// 字段赋值集合判定（`update_assignment`）：如实标出重复字段，不替调用方挑赢家。
    /// Assignment-set verdict (`update_assignment`): report duplicated fields instead of picking a winner.
    UpdateAssignments,
    /// 排序空值支持判定（`sort_by`）：后端能不能表达该排序项的空值排序。
    /// Sort null-ordering verdict (`sort_by`): whether the backend can express null ordering for an item.
    SortPlan,
    /// 仓储查询整体入口（`repository_api`）：基础计划 + where + 排序/分页 → 已校验计划。
    /// Whole repository-query entry (`repository_api`): base plan plus where plus sort/paging → a validated plan.
    RepositoryQuery,
    /// 管道注册计划（`model/pipeline`）：步骤顺序 + 约束组注册序列 + 同 id 不同名冲突。
    /// Pipeline registration plan (`model/pipeline`): step order, group registration sequence and id/name conflicts.
    PipelinePlan,
    /// 原 `network-scheduling` 的带时间窗车辆路径（分支定价）；领域模型由 `vrpProblem` 给出。
    /// Vehicle routing with time windows (branch and price) from the original `network-scheduling` crate; the domain
    /// model comes from `vrpProblem`.
    Vrp,
    /// 精确整数 CP，通过原生有限 MIP 降维 / Exact-integer CP through native finite MIP lowering.
    Cp,
    /// 一维物理装箱领域问题 / One-dimensional physical packing domain problem.
    Bpp1d,
    /// 二维物理装箱默认规则，求解仅在原生端。 / Default-rule two-dimensional packing, solved only natively.
    Bpp2d,
    /// 三维物理装箱默认长方体规则，单轮执行仅在原生端。 / Default-cuboid three-dimensional packing, whose one-round execution runs only natively.
    Bpp3d,
    /// 一维分切领域问题，执行仅在原生端。 / Cutting-stock domain problem, executed only natively.
    Csp1d,
    /// 二维分切默认规则，求解仅在原生端。 / Default-rule two-dimensional cutting stock, solved only natively.
    Csp2d,
    /// 共享 BPP 普通装载 application，实际执行仅在原生端。 / Shared BPP ordinary loading application, executed only natively.
    BppLoading,
    /// Gantt 活动调度有限候选域，单 MILP 执行仅在原生端。 / Gantt activity-scheduling finite-candidate domain, whose single-MILP execution runs only natively.
    GanttActivity,
}

/// 求解提交 / Solve submission.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SolveRequest {
    /// 协议主版本 / Protocol major version.
    pub protocol_version: u32,
    /// 后端或宿主预置策略 ID / Backend or host built-in strategy ID.
    pub solver: String,
    /// 无损模型 / Lossless model.
    pub model: ModelArtifact,
    /// 省略时沿用线性入口 / Omission preserves the linear entry point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model_kind: Option<ModelKind>,
    /// 毫秒时间限制 / Time limit in milliseconds.
    pub time_limit_ms: Option<u32>,
    /// 仅 CP 入口接受；省略保持既有默认值 / Accepted only by the CP entry; omission preserves existing defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cp_options: Option<ConstraintProgrammingOptions>,
    /// 仅 serial-linear 预置策略接受 / Accepted only by the built-in serial-linear strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub serial_options: Option<SerialLinearOptions>,
    /// 仅 parallel-linear 预置策略接受 / Accepted only by the built-in parallel-linear strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parallel_options: Option<ParallelLinearOptions>,
    /// 仅 `linearIis` 接受；省略沿用原 core 默认。 / Accepted only by `linearIis`; omission keeps the core defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub iis_options: Option<LinearIisOptions>,
    /// 仅 `linearSolutionPool` 接受；必须显式给出。 / Accepted only by `linearSolutionPool` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub solution_pool_options: Option<LinearSolutionPoolOptions>,
    /// 仅 `linearSensitivity` 接受；必须是完整且可行的变量向量。
    /// Accepted only by `linearSensitivity`; must be a complete and feasible variable vector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub incumbent_values: Option<Vec<f64>>,
    /// 仅 `linearSweep` 接受；必须显式给出。 / Accepted only by `linearSweep` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sweep_cases: Option<Vec<LinearSweepCase>>,
    /// 仅 `networkFlow` 接受；必须显式给出。 / Accepted only by `networkFlow` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub network_flow_problem: Option<NetworkFlowProblem>,
    /// 仅 `networkGeneralizedFlow` 接受；必须显式给出。 / Accepted only by `networkGeneralizedFlow` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub network_generalized_flow_problem: Option<GeneralizedFlowProblem>,
    /// 仅 `networkDesign` 接受；必须显式给出。 / Accepted only by `networkDesign` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub network_design_problem: Option<NetworkDesignProblem>,
    /// 仅 `networkStructure` 接受；必须显式给出。 / Accepted only by `networkStructure` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub network_structure_problem: Option<NetworkStructureProblem>,
    /// 仅 `networkAllocation` 接受；必须显式给出。 / Accepted only by `networkAllocation` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub allocation_problem: Option<AllocationProblem>,
    /// 仅 `networkResilience` 接受；必须显式给出。 / Accepted only by `networkResilience` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub network_resilience_problem: Option<NetworkResilienceProblem>,
    /// 仅 `vrp` 接受；必须显式给出。 / Accepted only by `vrp` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub vrp_problem: Option<VrpProblem>,
    /// 仅 `indexedRegistration` 接受；必须显式给出。 / Accepted only by `indexedRegistration` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub indexed_registration: Option<IndexedRegistrationRequest>,
    /// 仅 `linearColumnCertificate` 接受；批次与期望作用域必须成对给出。
    /// Accepted only by `linearColumnCertificate`; the batch and the expected scope must be given together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub column_batch: Option<LinearColumnBatch>,
    /// 与 `columnBatch` 配对；单独给出会被拒绝。 / Paired with `columnBatch`; giving it alone is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub column_scope: Option<LinearColumnScope>,
    /// 仅 `heartBeat` 接受；必须显式给出。 / Accepted only by `heartBeat` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub heart_beat: Option<HeartBeatRequest>,
    /// 仅 `responseEnvelope` 接受；由调用方给出的信封会被复核。
    /// Accepted only by `responseEnvelope`; a caller-provided envelope is rechecked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub response_envelope: Option<ResponseEnvelope>,
    /// 仅 `linearLifecycle` 接受；必须显式给出。 / Accepted only by `linearLifecycle` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub linear_lifecycle: Option<LinearLifecycleRequest>,
    /// 仅 `relationalQueryPlan` 接受；必须显式给出。 / Accepted only by `relationalQueryPlan` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub relational_query_plan: Option<RelationalQueryPlanSpec>,
    /// 仅 `linearBendersMaster` 接受；省略表示不带 cut 的裸主问题。
    /// Accepted only by `linearBendersMaster`; omission means a bare master with no cuts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub benders_cuts: Option<Vec<BendersCutSpec>>,
    /// 仅 `quadraticBendersMaster` 接受；省略表示不带二次 cut。
    /// Accepted only by `quadraticBendersMaster`; omission means no quadratic cuts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub benders_quadratic_cuts: Option<Vec<BendersQuadraticCutSpec>>,
    /// 仅 `linearBendersSub` 接受；必须显式给出（可为空）。
    /// Accepted only by `linearBendersSub` and required there (it may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub master_solution: Option<Vec<f64>>,
    /// 仅 `logicBasedBendersPlan` 接受；必须显式给出。
    /// Accepted only by `logicBasedBendersPlan` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub logic_based_benders_plan: Option<LogicBasedBendersPlanRequest>,
    /// 仅 `remoteArtifactCodec` 接受；必须显式给出。
    /// Accepted only by `remoteArtifactCodec` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub remote_artifact: Option<RemoteArtifactRequest>,
    /// 仅 `conflictExplanationVerify` 接受；必须显式给出（冲突解释的 JSON 文本）。
    /// Accepted only by `conflictExplanationVerify` and required there (the conflict explanation JSON text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub conflict_explanation: Option<String>,
    /// 仅 `candidateFunnelVerify` 接受；必须显式给出（候选漏斗排序的 JSON 文本）。
    /// Accepted only by `candidateFunnelVerify` and required there (the candidate funnel JSON text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub candidate_funnel: Option<String>,
    /// 仅 `candidateFunnelVerify` 使用；给出时输出短名单，省略则不给。
    /// Used only by `candidateFunnelVerify`; when present a shortlist is produced, otherwise none is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub shortlist_limit: Option<u32>,
    /// 仅 `effectivenessVerify` 接受；必须显式给出（有效性排序的 JSON 文本）。
    /// Accepted only by `effectivenessVerify` and required there (the effectiveness ranking JSON text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub effectiveness_ranking: Option<String>,
    /// 仅 `criticalAnalysisVerify` 接受；必须显式给出（关键约束总报告的 JSON 文本）。
    /// Accepted only by `criticalAnalysisVerify` and required there (the aggregate report JSON text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub critical_analysis: Option<String>,
    /// 仅 `relaxationRecommend` 接受；必须显式给出（冲突解释 + 业务成本 + 策略）。
    /// Accepted only by `relaxationRecommend` and required there (conflict explanation + costs + policy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub relaxation_request: Option<RelaxationRequest>,
    /// 仅 `fixedIntegerLpVerify` 使用；省略用领域默认容差。
    /// Used only by `fixedIntegerLpVerify`; absent means the domain default tolerances.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fixed_integer_config: Option<FixedIntegerConfigInput>,
    /// 仅 `fixedIntegerLpVerify` 使用；省略时只校验配置不投影报告。
    /// Used only by `fixedIntegerLpVerify`; when absent only the config is verified and no report is projected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fixed_integer_report: Option<String>,
    /// 仅 `perturbationPolicyVerify` 接受；必须显式给出（扰动策略 + 可选报告）。
    /// Accepted only by `perturbationPolicyVerify` and required there (policy plus an optional report).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub perturbation_policy: Option<PerturbationPolicyRequest>,
    /// 仅 `targetVerify` 接受；必须显式给出（目标的构造方式）。
    /// Accepted only by `targetVerify` and required there (how to build the target).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target_spec: Option<TargetSpec>,
    /// 仅 `targetVerify` 使用；给出时校验并投影可行性报告。
    /// Used only by `targetVerify`; when present the feasibility report is verified and projected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target_report: Option<String>,
    /// 仅 `multiTargetVerify` 接受；必须显式给出（多目标报告的 JSON 文本）。
    /// Accepted only by `multiTargetVerify` and required there (the multi-target report JSON text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub multi_target_report: Option<String>,
    /// 仅 `multiTargetVerify` 使用；省略时用领域默认策略。
    /// Used only by `multiTargetVerify`; absent means the domain default policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub relaxation_policy: Option<RelaxationPolicyInput>,
    /// 仅 `criticalityProfileVerify` 接受；必须显式给出（观察列表 + 可选的声称 profile）。
    /// Accepted only by `criticalityProfileVerify` and required there (observations plus an optional claimed profile).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub criticality_request: Option<CriticalityRequest>,
    /// 仅 `hyperHeuristicConfigVerify` 接受；必须显式给出（配置文本 + 格式）。
    /// Accepted only by `hyperHeuristicConfigVerify` and required there (config text plus format).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hyper_heuristic_config: Option<HyperHeuristicConfigRequest>,
    /// 仅 `rewardPolicyVerify` 接受；必须显式给出（至少一个奖励策略）。
    /// Accepted only by `rewardPolicyVerify` and required there (at least one reward policy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reward_policy: Option<RewardPolicyRequest>,
    /// 仅 `checkpointVerify` 接受；必须显式给出（文档类别 + 文本 + 可选的审计开关）。
    /// Accepted only by `checkpointVerify` and required there (kind, text and an optional audit flag).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub checkpoint_request: Option<CheckpointRequest>,
    /// 仅 `linearMappingVerify` 接受；省略表示不核对"声称的映射"。
    /// Accepted only by `linearMappingVerify`; omitting it means no claimed mapping is cross-checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub linear_mapping_request: Option<LinearMappingRequest>,
    /// 仅 `lbbResumeIdentityVerify` 接受；必须显式给出（恢复身份的全部字段）。
    /// Accepted only by `lbbResumeIdentityVerify` and required there (every resume-identity field).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lbb_resume_identity: Option<LbbResumeIdentityRequest>,
    /// 仅 `cpCheckpointVerify` 接受；必须显式给出（portable CP 检查点的规范 JSON 文本）。
    /// Accepted only by `cpCheckpointVerify` and required there (the canonical JSON text of a portable CP checkpoint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cp_checkpoint_request: Option<CpCheckpointRequest>,
    /// 仅 `bendersMetricsVerify` 接受；必须显式给出（指标文档）。
    /// Accepted only by `bendersMetricsVerify` and required there (the metrics document).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub benders_metrics: Option<BendersMetricsRequest>,
    /// 仅 `bendersCutVerify` 接受；必须显式给出（列数 + 线性/二次 cut）。
    /// Accepted only by `bendersCutVerify` and required there (column count plus linear and quadratic cuts).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub benders_cut_request: Option<BendersCutRequest>,
    /// 仅 `symbolConditionVerify` 接受；必须显式给出（关系 + 边界 + 范围）。
    /// Accepted only by `symbolConditionVerify` and required there (relation, boundary and range).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub symbol_condition: Option<SymbolConditionRequest>,
    /// 仅 `symbolLinearizationVerify` 接受；必须显式给出（big-M 与/或方法列表）。
    /// Accepted only by `symbolLinearizationVerify` and required there (a big-M and/or a method list).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub symbol_linearization: Option<SymbolLinearizationRequest>,
    /// 仅 `symbolCoreRelationsVerify` 接受；内容无意义（纯表读取），但必须显式给出以保持"每个 kind 有自己字段"的约定。
    /// Accepted only by `symbolCoreRelationsVerify`; its content is ignored (a pure table read) but must be present to keep
    /// the "each kind has its own field" convention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub symbol_core_relations: Option<SymbolCoreRelationsRequest>,
    /// 仅 `heuristicScheduleSample` 接受；必须显式给出（形态 + 参数 + 采样点数）。
    /// Accepted only by `heuristicScheduleSample` and required there (shape, parameters and sample count).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub heuristic_schedule: Option<HeuristicScheduleRequest>,
    /// 仅 `qTableSimulate` 接受；必须显式给出（维度 + 初始表 + 转移 + 选择器）。
    /// Accepted only by `qTableSimulate` and required there (dimensions, initial table, transitions and selector).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub q_table: Option<QTableRequest>,
    /// 仅 `masterCutEncodingVerify` 接受；必须显式给出（编码类型 + 赋值 + 族/行 + 可选冲突）。
    /// Accepted only by `masterCutEncodingVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub master_cut_encoding: Option<MasterCutEncodingRequest>,
    /// 仅 `analysisGroupAggregate` 接受；必须显式给出（三份报告 JSON 都可缺省，但字段本身要在）。
    /// Accepted only by `analysisGroupAggregate` and required there (each of the three report texts may be omitted, but the
    /// field itself must be present).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub analysis_group_aggregate: Option<AnalysisGroupAggregateRequest>,
    /// 仅 `predicateFieldResolutionVerify` 接受；必须显式给出。
    /// Accepted only by `predicateFieldResolutionVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub predicate_field_resolution: Option<PredicateFieldResolutionRequest>,
    /// 仅 `predicateFieldFilterExtract` 接受；必须显式给出（条件列表不能为空）。
    /// Accepted only by `predicateFieldFilterExtract` and required there (the condition list must not be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub predicate_field_filter: Option<PredicateFieldFilterRequest>,
    /// 仅 `modelConfigurationVerify` 接受；必须显式给出。
    /// Accepted only by `modelConfigurationVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model_configuration: Option<ModelConfigurationRequest>,
    /// 仅 `cacheKeyIdentityVerify` 接受；必须显式给出（键列表与操作序列都可为空）。
    /// Accepted only by `cacheKeyIdentityVerify` and required there (the key lists and the log may all be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cache_key_identity: Option<CacheKeyIdentityRequest>,
    /// 仅 `cacheKeyAddressVerify` 接受；必须显式给出（三组都可为空）。
    /// Accepted only by `cacheKeyAddressVerify` and required there (all three lists may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cache_key_address: Option<CacheKeyAddressRequest>,
    /// 仅 `flattenCacheVerify` 接受；必须显式给出（操作序列可为空）。
    /// Accepted only by `flattenCacheVerify` and required there (the operation log may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub flatten_cache: Option<FlattenCacheRequest>,
    /// 仅 `updateAssignmentVerify` 接受；必须显式给出（组列表可为空）。
    /// Accepted only by `updateAssignmentVerify` and required there (the group list may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub update_assignment: Option<UpdateAssignmentRequest>,
    /// 仅 `booleanScopeVerify` 接受；必须显式给出（步骤可为空，空作用域有恒等元）。
    /// Accepted only by `booleanScopeVerify` and required there (the steps may be empty, whose scope has an identity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub boolean_scope: Option<BooleanScopeRequest>,
    /// 仅 `unsupportedPredicateVerify` 接受；必须显式给出（两个列表都可为空）。
    /// Accepted only by `unsupportedPredicateVerify` and required there (both lists may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub unsupported_predicate: Option<UnsupportedPredicateRequest>,
    /// 仅 `repositoryCallVerify` 接受；必须显式给出。
    /// Accepted only by `repositoryCallVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub repository_call: Option<RepositoryCallRequest>,
    /// 仅 `functionUsageVerify` 接受；必须显式给出（用例可为空，此时只回报全格结论）。
    /// Accepted only by `functionUsageVerify` and required there (the cases may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub function_usage: Option<FunctionUsageRequest>,
    /// 仅 `inputDomainVerify` 接受；必须显式给出（三个列表都可为空）。
    /// Accepted only by `inputDomainVerify` and required there (all three lists may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub input_domain: Option<InputDomainRequest>,
    /// 仅 `nativeWriteVerify` 接受；必须显式给出（三个列表都可为空）。
    /// Accepted only by `nativeWriteVerify` and required there (all three lists may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub native_write: Option<NativeWriteRequest>,
    /// 仅 `elasticModelVerify` 接受；必须显式给出。
    /// Accepted only by `elasticModelVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub elastic_model: Option<ElasticModelRequest>,
    /// 仅 `combineStatusVerify` 接受；必须显式给出（序列可为空）。
    /// Accepted only by `combineStatusVerify` and required there (the sequence may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub combine_status: Option<CombineStatusRequest>,
    /// 仅 `numberTextVerify` 接受；必须显式给出（各列表都可为空）。
    /// Accepted only by `numberTextVerify` and required there (every list may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub number_text: Option<NumberTextRequest>,
    /// 仅 `bigMPolicyVerify` 接受；必须显式给出。
    /// Accepted only by `bigMPolicyVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub big_m_policy: Option<BigMPolicyRequest>,
    /// 仅 `conditionalSemanticsVerify` 接受；必须显式给出。
    /// Accepted only by `conditionalSemanticsVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub conditional_semantics: Option<ConditionalSemanticsRequest>,
    /// 仅 `searchBudgetVerify` 接受；必须显式给出。
    /// Accepted only by `searchBudgetVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub search_budget: Option<SearchBudgetRequest>,
    /// 仅 `predicateSchemaVerify` 接受；必须显式给出。
    /// Accepted only by `predicateSchemaVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub predicate_schema: Option<PredicateSchemaRequest>,
    /// 仅 `sortByVerify` 接受；必须显式给出。
    /// Accepted only by `sortByVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sort_by: Option<SortByRequest>,
    /// 仅 `variableKindDomainVerify` 接受；必须显式给出（探测表可为空）。
    /// Accepted only by `variableKindDomainVerify` and required there (the probe list may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub variable_kind_domain: Option<VariableKindDomainRequest>,
    /// 仅 `criticalAssemblyVerify` 接受；必须显式给出（场景表可为空）。
    /// Accepted only by `criticalAssemblyVerify` and required there (the case list may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub critical_assembly: Option<CriticalAssemblyRequest>,
    /// 仅 `flattenIdentityVerify` 接受；必须显式给出。
    /// Accepted only by `flattenIdentityVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub flatten_identity: Option<FlattenIdentityRequest>,
    /// 仅 `masterCutVerify` 接受；必须显式给出。
    /// Accepted only by `masterCutVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub master_cut: Option<MasterCutRequest>,
    /// 仅 `booleanFoldVerify` 接受；必须显式给出（字段表可为空）。
    /// Accepted only by `booleanFoldVerify` and required there (the field lists may be empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub boolean_fold: Option<BooleanFoldRequest>,
    /// 仅 `fieldResolutionVerify` 接受；必须显式给出。
    /// Accepted only by `fieldResolutionVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub field_resolution: Option<FieldResolutionRequest>,
    /// 仅 `scalarFunctionVerify` 接受；必须显式给出。
    /// Accepted only by `scalarFunctionVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub scalar_function: Option<ScalarFunctionRequest>,
    /// 仅 `polynomialLiftVerify` 接受；必须显式给出。
    /// Accepted only by `polynomialLiftVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub polynomial_lift: Option<PolynomialLiftRequest>,
    /// 仅 `boundedNoGoodVerify` 接受；必须显式给出。
    /// Accepted only by `boundedNoGoodVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bounded_no_good: Option<BoundedNoGoodRequest>,
    /// 仅 `effectivenessAggregateVerify` 接受；必须显式给出。
    /// Accepted only by `effectivenessAggregateVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub effectiveness_aggregate: Option<EffectivenessAggregateRequest>,
    /// 仅 `assumptionBindingVerify` 接受；必须显式给出。
    /// Accepted only by `assumptionBindingVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub assumption_binding: Option<AssumptionBindingRequest>,
    /// 仅 `tierClassificationVerify` 接受；必须显式给出。
    /// Accepted only by `tierClassificationVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tier_classification: Option<TierClassificationRequest>,
    /// 仅 `polarityEncodingVerify` 接受；必须显式给出。
    /// Accepted only by `polarityEncodingVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub polarity_encoding: Option<PolarityRequest>,
    /// 仅 `quadraticMonomialVerify` 接受；必须显式给出。
    /// Accepted only by `quadraticMonomialVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub quadratic_monomial: Option<MonomialSpec>,
    /// 仅 `deferredMaterializeVerify` 接受；必须显式给出。
    /// Accepted only by `deferredMaterializeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub deferred_materialize: Option<MaterializeRequest>,
    /// 仅 `backendFeaturesVerify` 接受；必须显式给出。
    /// Accepted only by `backendFeaturesVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub backend_features: Option<BackendFeaturesRequest>,
    /// 仅 `canonicalMonomialVerify` 接受；必须显式给出。
    /// Accepted only by `canonicalMonomialVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub canonical_monomial: Option<CanonicalMonomialRequest>,
    /// 仅 `expressionKeyVerify` 接受；必须显式给出。
    /// Accepted only by `expressionKeyVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expression_key: Option<ExpressionKeyRequest>,
    /// 仅 `encodingLabelVerify` 接受；必须显式给出。
    /// Accepted only by `encodingLabelVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub encoding_label: Option<EncodingLabelRequest>,
    /// 仅 `activityBandsVerify` 接受；必须显式给出。
    /// Accepted only by `activityBandsVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub activity_bands: Option<ActivityBandRequest>,
    /// 仅 `evidencePredicatesVerify` 接受；必须显式给出。
    /// Accepted only by `evidencePredicatesVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub evidence_predicates: Option<EvidenceSpecRequest>,
    /// 仅 `activityReportCheckVerify` 接受；必须显式给出。
    /// Accepted only by `activityReportCheckVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub activity_report_check: Option<ActivityReportRequest>,
    /// 仅 `auxiliaryDomainVerify` 接受；必须显式给出。
    /// Accepted only by `auxiliaryDomainVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub auxiliary_domain: Option<AuxiliaryDomainRequest>,
    /// 仅 `bindingAssignmentVerify` 接受；必须显式给出。
    /// Accepted only by `bindingAssignmentVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub binding_assignment: Option<BindingAssignmentRequest>,
    /// 仅 `sourceIdentityVerify` 接受；必须显式给出。
    /// Accepted only by `sourceIdentityVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub source_identity: Option<SourceIdentityRequest>,
    /// 仅 `cutValidityVerify` 接受；必须显式给出。
    /// Accepted only by `cutValidityVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cut_validity: Option<CutValidityRequest>,
    /// 仅 `assignmentCompositionVerify` 接受；必须显式给出。
    /// Accepted only by `assignmentCompositionVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub assignment_composition: Option<AssignmentCompositionRequest>,
    /// 仅 `dumpOptionsVerify` 接受；必须显式给出。
    /// Accepted only by `dumpOptionsVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dump_options: Option<DumpOptionsSpec>,
    /// 仅 `nativeWriteScopeVerify` 接受；必须显式给出。
    /// Accepted only by `nativeWriteScopeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub native_write_scope: Option<NativeWriteScopeRequest>,
    /// 仅 `writerRegistryVerify` 接受；必须显式给出。
    /// Accepted only by `writerRegistryVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub writer_registry: Option<WriterRegistryRequest>,
    /// 仅 `candidateShortlistVerify` 接受；必须显式给出。
    /// Accepted only by `candidateShortlistVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub candidate_shortlist: Option<ShortlistRequest>,
    /// 仅 `lazyFlattenInitVerify` 接受；必须显式给出。
    /// Accepted only by `lazyFlattenInitVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lazy_flatten_init: Option<LazyInitRequest>,
    /// 仅 `dumpBatchVerify` 接受；必须显式给出。
    /// Accepted only by `dumpBatchVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dump_batch: Option<DumpBatchRequest>,
    /// 仅 `quadraticElasticVerify` 接受；必须显式给出。
    /// Accepted only by `quadraticElasticVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub quadratic_elastic: Option<QuadraticElasticRequest>,
    /// 仅 `stableValueVerify` 接受；必须显式给出。
    /// Accepted only by `stableValueVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stable_value: Option<StableValueRequest>,
    /// 仅 `masterAssignmentVerify` 接受；必须显式给出。
    /// Accepted only by `masterAssignmentVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub master_assignment: Option<MasterAssignmentRequest>,
    /// 仅 `sparseTriadVerify` 接受；必须显式给出。
    /// Accepted only by `sparseTriadVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sparse_triad: Option<SparseTriadRequest>,
    /// 仅 `matrixRowsVerify` 接受；必须显式给出。
    /// Accepted only by `matrixRowsVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub matrix_rows: Option<MatrixRowsRequest>,
    /// 仅 `polynomialProbeVerify` 接受；必须显式给出。
    /// Accepted only by `polynomialProbeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub polynomial_probe: Option<PolynomialProbeRequest>,
    /// 仅 `modelViewVerify` 接受；必须显式给出。
    /// Accepted only by `modelViewVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub model_view: Option<ModelViewRequest>,
    /// 仅 `analysisOptionsVerify` 接受；必须显式给出。
    /// Accepted only by `analysisOptionsVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub analysis_options: Option<AnalysisOptionsRequest>,
    /// 仅 `capabilityMatrixVerify` 接受；必须显式给出。
    /// Accepted only by `capabilityMatrixVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub capability_matrix: Option<CapabilityMatrixRequest>,
    /// 仅 `cpSupportMergeVerify` 接受；必须显式给出。
    /// Accepted only by `cpSupportMergeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cp_support_merge: Option<CpSupportMergeRequest>,
    /// 仅 `subproblemResultVerify` 接受；必须显式给出。
    /// Accepted only by `subproblemResultVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub subproblem_result: Option<SubproblemResultRequest>,
    /// 仅 `noGoodOracleVerify` 接受；必须显式给出。
    /// Accepted only by `noGoodOracleVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub no_good_oracle: Option<NoGoodOracleRequest>,
    /// 仅 `cpModelBuildVerify` 接受；必须显式给出。
    /// Accepted only by `cpModelBuildVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub cp_model_build: Option<CpModelBuildRequest>,
    /// 仅 `snapshotGateVerify` 接受；必须显式给出。
    /// Accepted only by `snapshotGateVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub snapshot_gate: Option<SnapshotGateRequest>,
    /// 仅 `constraintCanonVerify` 接受；必须显式给出。
    /// Accepted only by `constraintCanonVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub constraint_canon: Option<ConstraintCanonRequest>,
    /// 仅 `assignmentEvalVerify` 接受；必须显式给出。
    /// Accepted only by `assignmentEvalVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub assignment_eval: Option<AssignmentEvalRequest>,
    /// 仅 `expressionBoundsVerify` 接受；必须显式给出。
    /// Accepted only by `expressionBoundsVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expression_bounds: Option<ExpressionBoundsRequest>,
    /// 仅 `domainSurfaceVerify` 接受；必须显式给出。
    /// Accepted only by `domainSurfaceVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub domain_surface: Option<DomainProbeRequest>,
    /// 仅 `identityWrapVerify` 接受；必须显式给出。
    /// Accepted only by `identityWrapVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub identity_wrap: Option<IdentityWrapRequest>,
    /// 仅 `containerSurfaceVerify` 接受；必须显式给出。
    /// Accepted only by `containerSurfaceVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub container_surface: Option<ContainerProbeRequest>,
    /// 仅 `activationSetVerify` 接受；必须显式给出。
    /// Accepted only by `activationSetVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub activation_set: Option<ActivationSetRequest>,
    /// 仅 `tierSelectionVerify` 接受；必须显式给出。
    /// Accepted only by `tierSelectionVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tier_selection: Option<TierSelectionRequest>,
    /// 仅 `evidenceRemapVerify` 接受；必须显式给出。
    /// Accepted only by `evidenceRemapVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub evidence_remap: Option<EvidenceRemapRequest>,
    /// 仅 `dominanceVerify` 接受；必须显式给出。
    /// Accepted only by `dominanceVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dominance: Option<DominanceRequest>,
    /// 仅 `dispersionVerify` 接受；必须显式给出。
    /// Accepted only by `dispersionVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub dispersion: Option<DispersionRequest>,
    /// 仅 `referencePointsVerify` 接受；必须显式给出。
    /// Accepted only by `referencePointsVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub reference_points: Option<ReferencePointsRequest>,
    /// 仅 `paretoArchiveVerify` 接受；必须显式给出。
    /// Accepted only by `paretoArchiveVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pareto_archive: Option<ArchiveRequest>,
    /// 仅 `constraintRankingVerify` 接受；必须显式给出。
    /// Accepted only by `constraintRankingVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub constraint_ranking: Option<ConstraintRankingRequest>,
    /// 仅 `constraintPriorityVerify` 接受；必须显式给出。
    /// Accepted only by `constraintPriorityVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub constraint_priority: Option<PriorityRequest>,
    /// 仅 `populationConfigVerify` 接受；必须显式给出。
    /// Accepted only by `populationConfigVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub population_config: Option<PopulationConfigRequest>,
    /// 仅 `boundaryVerify` 接受；必须显式给出。
    /// Accepted only by `boundaryVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub boundary: Option<BoundaryRequest>,
    /// 仅 `acceptanceVerify` 接受；必须显式给出。
    /// Accepted only by `acceptanceVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub acceptance: Option<AcceptanceRequest>,
    /// 仅 `rankingProbeVerify` 接受；必须显式给出。
    /// Accepted only by `rankingProbeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ranking_probe: Option<RankingProbeRequest>,
    /// 仅 `topologyVerify` 接受；必须显式给出。
    /// Accepted only by `topologyVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub topology: Option<TopologyRequest>,
    /// 仅 `migrationVerify` 接受；必须显式给出。
    /// Accepted only by `migrationVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub migration: Option<MigrationRequest>,
    /// 仅 `archiveProbeVerify` 接受；必须显式给出。
    /// Accepted only by `archiveProbeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub archive_probe: Option<ArchiveProbeRequest>,
    /// 仅 `piecewiseVerify` 接受；必须显式给出。
    /// Accepted only by `piecewiseVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub piecewise: Option<PiecewiseRequest>,
    /// 仅 `diversityVerify` 接受；必须显式给出。
    /// Accepted only by `diversityVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub diversity: Option<DiversityRequest>,
    /// 仅 `solutionStatusVerify` 接受；必须显式给出。
    /// Accepted only by `solutionStatusVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub solution_status: Option<SolutionStatusRequest>,
    /// 仅 `funnelVerify` 接受；必须显式给出。
    /// Accepted only by `funnelVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub funnel: Option<FunnelRequest>,
    /// 仅 `effectivenessProbeVerify` 接受；必须显式给出。
    /// Accepted only by `effectivenessProbeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub effectiveness_probe: Option<EffectivenessProbeRequest>,
    /// 仅 `criticalReportVerify` 接受；必须显式给出。
    /// Accepted only by `criticalReportVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub critical_report: Option<CriticalReportRequest>,
    /// 仅 `groupSummaryVerify` 接受；必须显式给出。
    /// Accepted only by `groupSummaryVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub group_summary: Option<GroupSummaryRequest>,
    /// 仅 `bandProbeVerify` 接受；必须显式给出。
    /// Accepted only by `bandProbeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub band_probe: Option<BandProbeRequest>,
    /// 仅 `evidenceProbeVerify` 接受；必须显式给出。
    /// Accepted only by `evidenceProbeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub evidence_probe: Option<EvidenceProbeRequest>,
    /// 仅 `proofProbeVerify` 接受；必须显式给出。
    /// Accepted only by `proofProbeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub proof_probe: Option<ProofProbeRequest>,
    /// 仅 `gapProbeVerify` 接受；必须显式给出。
    /// Accepted only by `gapProbeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub gap_probe: Option<GapProbeRequest>,
    /// 仅 `diagnosticsProbeVerify` 接受；必须显式给出。
    /// Accepted only by `diagnosticsProbeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub diagnostics_probe: Option<DiagnosticsProbeRequest>,
    /// 仅 `fingerprintProbeVerify` 接受；必须显式给出。
    /// Accepted only by `fingerprintProbeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fingerprint_probe: Option<FingerprintProbeRequest>,
    /// 仅 `blockingSetVerify` 接受；必须显式给出。
    /// Accepted only by `blockingSetVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub blocking_set: Option<BlockingSetRequest>,
    /// 仅 `correctionProbeVerify` 接受；必须显式给出。
    /// Accepted only by `correctionProbeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub correction_probe: Option<CorrectionProbeRequest>,
    /// 仅 `sensitivityProbeVerify` 接受；必须显式给出。
    /// Accepted only by `sensitivityProbeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sensitivity_probe: Option<SensitivityProbeRequest>,
    /// 仅 `funnelRankingVerify` 接受；必须显式给出。
    /// Accepted only by `funnelRankingVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub funnel_ranking: Option<FunnelProbeRequest>,
    /// 仅 `effectivenessRankingVerify` 接受；必须显式给出。
    /// Accepted only by `effectivenessRankingVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub effectiveness_ranking_probe: Option<EffectivenessRankingRequest>,
    /// 仅 `persistenceSortVerify` 接受；必须显式给出。
    /// Accepted only by `persistenceSortVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub persistence_sort_probe: Option<PersistenceSortProbeRequest>,
    /// 仅 `persistenceSchemaVerify` 接受；必须显式给出。
    /// Accepted only by `persistenceSchemaVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub persistence_schema_probe: Option<PersistenceSchemaRequest>,
    /// 仅 `predicateScopeVerify` 接受；必须显式给出。
    /// Accepted only by `predicateScopeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub predicate_scope_probe: Option<PredicateScopeRequest>,
    /// 仅 `nativeTerminalProofVerify` 接受；必须显式给出。
    /// Accepted only by `nativeTerminalProofVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub native_terminal_proof_probe: Option<NativeTerminalProofRequest>,
    /// 仅 `nativeLoweringVerify` 接受；必须显式给出。
    /// Accepted only by `nativeLoweringVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub native_lowering_probe: Option<NativeLoweringProbeRequest>,
    /// 仅 `solveValueVerify` 接受；必须显式给出。
    /// Accepted only by `solveValueVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub solve_value_probe: Option<SolveValueProbeRequest>,
    /// 仅 `selectionPolicyVerify` 接受；必须显式给出。
    /// Accepted only by `selectionPolicyVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub selection_policy_probe: Option<SelectionProbeRequest>,
    /// 仅 `nonStationaryVerify` 接受；必须显式给出。
    /// Accepted only by `nonStationaryVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub non_stationary_probe: Option<NonStationaryProbeRequest>,
    /// 仅 `trainingSplitVerify` 接受；必须显式给出。
    /// Accepted only by `trainingSplitVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub training_split_probe: Option<TrainingSplitProbeRequest>,
    /// 仅 `acceptanceRecoveryVerify` 接受；必须显式给出。
    /// Accepted only by `acceptanceRecoveryVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub acceptance_recovery_probe: Option<AcceptanceRecoveryRequest>,
    /// 仅 `hypervolumeVerify` 接受；必须显式给出。
    /// Accepted only by `hypervolumeVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub hypervolume_probe: Option<HypervolumeProbeRequest>,
    /// 仅 `operatorCapabilityVerify` 接受；必须显式给出。
    /// Accepted only by `operatorCapabilityVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub operator_capability_probe: Option<OperatorCapabilityRequest>,
    /// 仅 `scheduleLogVerify` 接受；必须显式给出。
    /// Accepted only by `scheduleLogVerify` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub schedule_log_probe: Option<ScheduleLogRequest>,
    /// 仅 `predicateFieldFilters` 接受；必须显式给出。
    /// Accepted only by `predicateFieldFilters` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub predicate: Option<QueryFilter>,
    /// 仅 `updateAssignments` 接受；省略表示空赋值集合。 / Accepted only by `updateAssignments`; omission means empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub assignments: Option<Vec<AssignmentSpec>>,
    /// 仅 `sortPlan` 接受；必须显式给出。 / Accepted only by `sortPlan` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub sort_plan: Option<SortPlanSpec>,
    /// 仅 `repositoryQuery` 接受；必须显式给出。 / Accepted only by `repositoryQuery` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub repository_query: Option<RepositoryQuerySpec>,
    /// 仅 `pipelinePlan` 接受；省略表示空管道列表。 / Accepted only by `pipelinePlan`; omission means an empty list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pipeline_steps: Option<Vec<PipelineStepSpec>>,
    /// 仅 `quadraticOrchestration` 接受；必须显式给出。 / Accepted only by `quadraticOrchestration` and required there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub quadratic_orchestration: Option<QuadraticOrchestrationOptions>,
    /// 仅 fixed-recourse 预置策略接受 / Accepted only by the built-in fixed-recourse strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub fixed_recourse_options: Option<FixedRecourseOptions>,
    /// 仅 column-generation 预置策略接受 / Accepted only by the built-in column-generation strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub column_generation_options: Option<ColumnGenerationOptions>,
    /// 仅 bpp1d 领域入口接受 / Accepted only by the BPP1D domain entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bpp1d_options: Option<Bpp1dOptions>,
    /// 仅 csp1d 领域策略接受。 / Accepted only by the CSP1D domain strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub csp1d_options: Option<Csp1dOptions>,
    /// 仅 bpp2d 原生列生成策略接受。 / Accepted only by the BPP2D native column-generation strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bpp2d_options: Option<Bpp2dOptions>,
    /// 仅 bpp3d 原生单轮执行策略接受。 / Accepted only by the BPP3D native one-round execution strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bpp3d_options: Option<Bpp3dOptions>,
    /// 仅 csp2d 原生列生成策略接受。 / Accepted only by the CSP2D native column-generation strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub csp2d_options: Option<Csp2dOptions>,
    /// 仅共享 BPP 普通 application 接受。 / Accepted only by the shared BPP ordinary application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub bpp_loading_options: Option<BppLoadingOptions>,
    /// 仅 scip／gurobi 直接入口接受的性能调优；领域与框架策略各自持有预算和配置。
    /// Accepted only by the direct scip/gurobi entries; domain and framework strategies own their budgets and configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tuning_options: Option<SolverTuningOptions>,
    /// 仅 gantt-activity 原生策略接受。 / Accepted only by the native Gantt activity strategy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub gantt_activity_options: Option<GanttActivityOptions>,
}

/// 后端性能调优参数；只有显式 `mip_gap` 会影响终止原因（可能以 `GapLimit` 结束且无最优性证明）。
/// Backend performance tuning with only cross-backend deterministic switches; it never carries thresholds that weaken optimality or feasibility claims.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SolverTuningOptions {
    /// 求解线程数，1..=64（SCIP 与 Gurobi 都接受的上界）；缺省由后端决定。
    /// Solver threads within 1..=64, the shared bound both SCIP and Gurobi accept; the backend decides when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub threads: Option<u32>,
    /// 随机种子，用于可复现的重复运行。 / Random seed for reproducible repeated runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub seed: Option<i32>,
    /// 相对 MIP 间隙，0 表示精确最优；非零间隙可能以 `GapLimit` 终止且不携带最优性证明。
    /// Relative MIP gap, where zero means exact optimality; a nonzero gap may terminate with `GapLimit` and no optimality proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub mip_gap: Option<f64>,
}

impl SolverTuningOptions {
    /// 复验取值范围；不接受零线程或空对象。 / Revalidate the value ranges; zero threads or an empty object are rejected.
    pub fn validate(&self) -> Result<()> {
        if self.threads.is_none() && self.seed.is_none() && self.mip_gap.is_none() {
            return Err(BridgeError::new("InvalidInput", "solver tuning requires threads, seed or mipGap"));
        }
        if self.mip_gap.is_some_and(|gap| !gap.is_finite() || gap < 0.0 || gap >= 1.0) {
            return Err(BridgeError::new("InvalidInput", "solver tuning mipGap must be finite and within 0.0..1.0"));
        }
        // SCIP 的 `lp/threads` 上限为 64，超出会以 ParameterWrongVal 失败；共享边界必须取下界最紧的那个。
        // SCIP caps `lp/threads` at 64 and fails with ParameterWrongVal beyond it, so the shared bound is the tightest one.
        if self.threads.is_some_and(|threads| threads == 0 || threads > 64) {
            return Err(BridgeError::new("InvalidInput", "solver tuning threads must be within 1..=64"));
        }
        Ok(())
    }
}

/// 后端描述 / Backend description.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BackendDescriptor {
    /// 后端 ID / Backend ID.
    pub id: String,
    /// 已编译但仍需运行时环境 / Compiled, but still requires runtime prerequisites.
    pub compiled: bool,
    /// 已验证的桥接能力 / Verified bridge capabilities.
    pub capabilities: Vec<String>,
    /// 原生运行时条件 / Native runtime prerequisites.
    pub warnings: Vec<String>,
}

/// 宿主能力 / Host capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HostCapabilities {
    /// 协议主版本 / Protocol major version.
    pub protocol_version: u32,
    /// 后端列表 / Backend list.
    pub backends: Vec<BackendDescriptor>,
}

/// 任务状态 / Job status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum JobStatus {
    /// 正在求解 / Running.
    Running,
    /// 报告已完成（含取消和不可行）/ Report completed, including cancellation and infeasibility.
    Completed,
    /// 执行失败 / Execution failed.
    Failed,
}

/// 任务视图，报告和进度保留原始 JSON / Job view retaining raw report and progress JSON.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    /// 任务 ID / Job ID.
    pub id: String,
    /// 协议主版本 / Protocol major version.
    pub protocol_version: u32,
    /// 任务状态 / Job status.
    pub status: JobStatus,
    /// 模型指纹 / Model fingerprint.
    pub fingerprint: String,
    /// 编排返回对应的无损报告，其余任务返回统一报告 / Orchestration returns its corresponding lossless report; other jobs return unified reports.
    pub report_json: Option<String>,
    /// 最新进度 JSON / Latest progress JSON.
    pub progress_json: Option<String>,
    /// 结构化错误 / Structured error.
    pub error: Option<BridgeError>,
}

/// 从统一报告读取的 f64 摘要 / f64 summary read from a unified report.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReportSummary {
    /// 问题结论 / Problem conclusion.
    pub problem_status: String,
    /// 终止原因 / Termination reason.
    pub termination_reason: String,
    /// 目标值 / Objective value.
    pub objective_value: Option<f64>,
    /// 求解器顺序的变量值 / Variable values in solver order.
    pub values: Vec<f64>,
    /// 是否携带已验证的最优性证明；非零间隙终止时为 false。
    /// Whether a verified optimality proof is attached; false for a nonzero-gap termination.
    pub has_optimality_proof: bool,
    /// 相对 gap / Relative gap.
    pub relative_gap: Option<f64>,
    /// 绝对 gap / Absolute gap.
    pub absolute_gap: Option<f64>,
    /// 分支定界节点数；JSON 数字，超过 2^53 会丢精度。 / Branch-and-bound node count as a JSON number, which loses precision beyond 2^53.
    #[ts(type = "number | null")]
    pub nodes: Option<u64>,
    /// 求解耗时（毫秒）；JSON 数字 / Solve duration in milliseconds as a JSON number.
    #[ts(type = "number")]
    pub solve_time_ms: u64,
}

/// 通过 Rust 校验并读取报告 / Validate and read a report through Rust.
pub fn summarize_report(json: &str) -> Result<ReportSummary> {
    let report: ospf_rust_core::solver::SolveReport<f64, u64> = serde_json::from_str(json).map_err(invalid)?;
    if report.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported report schema")); }
    report.validate().map_err(invalid)?;
    // 证明只在 kind=Optimality 且 status=Verified 时算数；其余一律视为没有证明。
    // A proof only counts when kind is Optimality and status is Verified; anything else is no proof.
    let has_optimality_proof = report.proof.as_ref().is_some_and(|proof| {
        matches!(proof.kind, ospf_rust_core::solver::report::ProofKind::Optimality)
            && matches!(proof.status, ospf_rust_core::solver::report::ProofStatus::Verified)
    });
    let statistics = &report.statistics;
    Ok(ReportSummary {
        problem_status: serde_json::to_value(report.problem_status).map_err(invalid)?.as_str().unwrap_or_default().into(),
        termination_reason: serde_json::to_value(report.termination_reason).map_err(invalid)?.as_str().unwrap_or_default().into(),
        objective_value: report.solution.as_ref().and_then(|s| s.objective_value.or(s.objective)),
        values: report.solution.map(|s| s.values).unwrap_or_default(),
        has_optimality_proof,
        relative_gap: statistics.relative_gap,
        absolute_gap: statistics.absolute_gap,
        nodes: statistics.nodes,
        solve_time_ms: statistics.solve_time.as_millis().min(u128::from(u64::MAX)) as u64,
    })
}

fn invalid(message: impl ToString) -> BridgeError {
    BridgeError::new("InvalidInput", message)
}

/// 校验桥接变量边界并转换无界值。 / Validate bridge variable bounds and convert unbounded values.
pub fn linear_variable_range(
    variable_type: SerializedVariableType,
    lower_bound: f64,
    upper_bound: f64,
) -> Result<VariableRange<f64>> {
    if lower_bound.is_nan() || upper_bound.is_nan() || lower_bound > upper_bound
        || lower_bound == f64::INFINITY || upper_bound == f64::NEG_INFINITY
    {
        return Err(invalid("invalid variable bounds"));
    }
    let lower = lower_bound.is_finite().then_some(lower_bound);
    let upper = upper_bound.is_finite().then_some(upper_bound);
    match variable_type {
        SerializedVariableType::Continuous => Ok(VariableRange::new(lower, upper)),
        SerializedVariableType::Binary => {
            if !matches!(lower, Some(0.0 | 1.0)) || !matches!(upper, Some(0.0 | 1.0)) {
                return Err(invalid("binary bounds must be 0 or 1, including fixed binary variables"));
            }
            Ok(VariableRange::new(lower, upper))
        }
        SerializedVariableType::Integer => {
            let exact = |value: Option<f64>| -> Result<Option<f64>> {
                value.map(|value| {
                    if value.fract() != 0.0 || value.abs() > 9_007_199_254_740_991.0 {
                        Err(invalid("integer bounds exceed the exact f64 bridge range"))
                    } else { Ok(value) }
                }).transpose()
            };
            Ok(VariableRange::new(exact(lower)?, exact(upper)?))
        }
        _ => Err(BridgeError::new("Unsupported", "unsupported variable type")),
    }
}

/// 校验并注册桥接支持的变量，保留模型中的 Rust 变量身份。
/// Validate and register a bridge-supported variable, preserving its Rust identity in the model.
pub fn register_linear_variable(model: &mut MetaModel<f64>, variable: &SerializedVariable) -> Result<usize> {
    let index = model.tokens().len();
    if variable.index != index || variable.name.trim().is_empty() {
        return Err(invalid("invalid variable index or name"));
    }
    let range = linear_variable_range(variable.variable_type, variable.lower_bound, variable.upper_bound)?;
    match variable.variable_type {
        SerializedVariableType::Continuous => model.add_variable(ContinuousVariableItem::auto_with_range(&variable.name, range)).map_err(invalid),
        SerializedVariableType::Binary => model.add_variable(BinaryVariableItem::auto_with_range(&variable.name, range)).map_err(invalid),
        SerializedVariableType::Integer => model.add_variable(IntegerVariableItem::auto_with_range(&variable.name, range)).map_err(invalid),
        _ => Err(BridgeError::new("Unsupported", "unsupported variable type")),
    }
}

/// 将现有远程模型还原为 MetaModel，不接受静默类型降级。
/// Rebuild the existing remote model as a MetaModel without silent type degradation.
pub fn compile_linear(input: &SerializedLinearModel) -> Result<LinearTriadModel> {
    compile_linear_meta(input)?.try_into_linear_triad_model().map_err(invalid)
}

/// 复用同一校验与注册路径还原完整线性 MetaModel / Rebuild a full linear MetaModel through the same validation and registration path.
pub fn compile_linear_meta(input: &SerializedLinearModel) -> Result<MetaModel<f64>> {
    input.validate_contract().map_err(invalid)?;
    if input.schema_version != "1.0" || input.name.trim().is_empty() {
        return Err(invalid("expected a named schema 1.0 model"));
    }
    let mut model = MetaModel::<f64>::new(&input.name);
    for variable in &input.variables { register_linear_variable(&mut model, variable)?; }
    let count = input.variables.len();
    let mut objective = vec![0.0; count];
    if input.objective.constant != 0.0 {
        return Err(BridgeError::new("Unsupported", "nonzero objective constants are not yet supported"));
    }
    for cell in &input.objective.cells {
        if cell.col_index >= count || !cell.coefficient.is_finite() {
            return Err(invalid("invalid objective cell"));
        }
        objective[cell.col_index] += cell.coefficient;
        if !objective[cell.col_index].is_finite() { return Err(invalid("objective coefficient overflow")); }
    }
    let category = match input.objective.category {
        SerializedObjectiveCategory::Minimize => ObjectiveCategory::Minimum,
        SerializedObjectiveCategory::Maximize => ObjectiveCategory::Maximum,
        _ => return Err(invalid("unknown objective category")),
    };
    model.set_linear_objective(objective, category);
    for (row, constraint) in input.constraints.iter().enumerate() {
        if !constraint.rhs.is_finite() { return Err(invalid("non-finite constraint RHS")); }
        let mut terms = vec![0.0; count];
        for cell in &constraint.cells {
            if cell.row_index != row || cell.col_index >= count || !cell.coefficient.is_finite() {
                return Err(invalid("invalid constraint cell"));
            }
            terms[cell.col_index] += cell.coefficient;
            if !terms[cell.col_index].is_finite() { return Err(invalid("constraint coefficient overflow")); }
        }
        let relation = match constraint.sign {
            SerializedConstraintSign::LessEqual => ConstraintRelation::LessEqual,
            SerializedConstraintSign::GreaterEqual => ConstraintRelation::GreaterEqual,
            SerializedConstraintSign::Equal => ConstraintRelation::Equal,
            _ => return Err(invalid("unknown constraint sign")),
        };
        let terms: Vec<_> = terms.into_iter().enumerate().filter(|(_, v)| *v != 0.0).collect();
        model.add_linear_constraint(&terms, relation, constraint.rhs, &constraint.name).map_err(invalid)?;
    }
    Ok(model)
}

/// 无损编码现有模型协议，无穷边界使用字符串。
/// Encode the existing model protocol losslessly, representing infinite bounds as strings.
/// 往返时无法保留的模型返回 Unsupported；独立子模型可使用 compiled-linear codec。
/// Models that cannot round-trip return Unsupported; use the compiled-linear codec for independent child models.
pub fn encode_model(model: &LinearTriadModel) -> Result<ModelArtifact> {
    let fingerprint = linear_model_fingerprint(model).map_err(invalid)?.value;
    let serialized = serialize_linear_model(model);
    let decoded = compile_linear(&serialized).map_err(|err| BridgeError::new("Unsupported", format!("linear model cannot be preserved by this codec: {err}")))?;
    if linear_model_fingerprint(&decoded).map_err(invalid)?.value != fingerprint || model.basic.var_types != decoded.basic.var_types {
        return Err(BridgeError::new("Unsupported", "linear model contains information not preserved by this codec"));
    }
    Ok(ModelArtifact { model_json: serde_json::to_string(&serialized).map_err(invalid)?, fingerprint })
}

/// 校验协议、载荷和模型指纹 / Validate protocol, payload, and model fingerprint.
pub fn decode_model(artifact: &ModelArtifact) -> Result<LinearTriadModel> {
    let input: SerializedLinearModel = serde_json::from_str(&artifact.model_json).map_err(invalid)?;
    let model = compile_linear(&input)?;
    if linear_model_fingerprint(&model).map_err(invalid)?.value != artifact.fingerprint {
        return Err(BridgeError::new("FingerprintMismatch", "model fingerprint mismatch"));
    }
    Ok(model)
}

/// 无损导出实际子模型，保留松弛类型及元数据，不改写源报告身份。
/// Losslessly export an actual child model, retaining relaxed types and metadata without rewriting source-report identity.
pub fn encode_compiled_model(model: &LinearTriadModel) -> Result<ModelArtifact> {
    let compiled = ospf_rust_framework::solver::remote::encode_compiled_linear_model(model).map_err(invalid)?;
    let model_json = serde_json::to_string(&compiled).map_err(invalid)?;
    if model_json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "compiled model transport exceeds 8 MiB")); }
    Ok(ModelArtifact { model_json, fingerprint: compiled.content_fingerprint.value })
}

/// 显式已编译模型门禁；传输摘要覆盖原 codec 指纹以外的 solver 类型。
/// Explicit compiled-model gate; the transport digest covers solver types beyond the original core fingerprint.
pub fn decode_compiled_model(artifact: &ModelArtifact) -> Result<LinearTriadModel> {
    if artifact.model_json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "compiled model transport exceeds 8 MiB")); }
    let compiled: ospf_rust_framework::solver::remote::CompiledLinearModelArtifact = serde_json::from_str(&artifact.model_json).map_err(invalid)?;
    if compiled.schema_version != "1.0" { return Err(BridgeError::new("UnsupportedProtocolVersion", "unsupported compiled model transport schema")); }
    if compiled.content_fingerprint.value != artifact.fingerprint { return Err(BridgeError::new("FingerprintMismatch", "compiled model transport fingerprint mismatch")); }
    ospf_rust_framework::solver::remote::decode_compiled_linear_model(&compiled).map_err(invalid)
}

/// 为原生编排还原完整模型，并校验既有交换指纹 / Restore the complete model for native orchestration and verify its existing exchange fingerprint.
pub fn decode_meta_model(artifact: &ModelArtifact) -> Result<MetaModel<f64>> {
    if artifact.model_json.len() > 8 * 1024 * 1024 { return Err(BridgeError::new("PayloadTooLarge", "model exceeds 8 MiB")); }
    let input: SerializedLinearModel = serde_json::from_str(&artifact.model_json).map_err(invalid)?;
    let model = compile_linear_meta(&input)?;
    let compiled = model.try_to_linear_triad_model().map_err(invalid)?;
    if linear_model_fingerprint(&compiled).map_err(invalid)?.value != artifact.fingerprint {
        return Err(BridgeError::new("FingerprintMismatch", "model fingerprint mismatch"));
    }
    Ok(model)
}

/// 生成 SDK 协议声明 / Generate SDK protocol declarations.
pub fn typescript_declarations() -> String {
    [BridgeError::decl(), ModelArtifact::decl(), ModelKind::decl(), SolveRequest::decl(), SolverTuningOptions::decl(), BackendDescriptor::decl(),
        ConstraintOptions::decl(), ActivityOptions::decl(), ActivityStatus::decl(), AnalysisCandidateTier::decl(), AnalysisCandidate::decl(), AnalysisCandidateFunnel::decl(), LinearIisAlgorithm::decl(), LinearIisOptions::decl(), LinearIisReport::decl(), LinearSolutionPoolOptions::decl(), LinearSolutionPoolReport::decl(), CertifiedPricingDual::decl(), LinearPricingDuals::decl(), LinearSweepCase::decl(), LinearSweepCaseResult::decl(), LinearSweepReport::decl(), NetworkFlowNode::decl(), NetworkFlowArc::decl(), NetworkFlowNodeQuantity::decl(), NetworkFlowArcQuantity::decl(), NetworkFlowCommodity::decl(), NetworkFlowProblem::decl(), NetworkFlowQuantity::decl(), NetworkFlowArcFlow::decl(), NetworkFlowNamedQuantity::decl(), NetworkFlowSolution::decl(), NetworkFlowReport::decl(), GeneralizedFlowNode::decl(), GeneralizedFlowArc::decl(), GeneralizedFlowProblem::decl(), GeneralizedFlowArcResult::decl(), GeneralizedFlowNodeResult::decl(), GeneralizedFlowSolution::decl(), GeneralizedFlowReport::decl(), NetworkDesignLevel::decl(), NetworkDesignService::decl(), NetworkDesignFacility::decl(), NetworkDesignProblem::decl(), NetworkServiceLevel::decl(), NetworkConstruction::decl(), NetworkDesignSolution::decl(), NetworkDesignReport::decl(), NetworkStructurePartition::decl(), NetworkStructureQuota::decl(), NetworkStructureProblem::decl(), NetworkParentArc::decl(), NetworkPartitionOwner::decl(), NetworkStructureSolution::decl(), NetworkStructureReport::decl(), AllocationNamedQuantity::decl(), AllocationLocationSpec::decl(), AllocationServiceSpec::decl(), AllocationFlowQuantity::decl(), AllocationRequestSpec::decl(), AllocationQuotaSpec::decl(), AllocationHubSpec::decl(), AllocationOptionsSpec::decl(), AllocationProblem::decl(), AllocationShipment::decl(), AllocationSolution::decl(), AllocationReport::decl(), ResilienceDisjointness::decl(), ResilienceConnectivity::decl(), ResilienceArcQuantity::decl(), ResilienceCommodityBalances::decl(), ResilienceNodeSlack::decl(), ResilienceCommoditySlacks::decl(), ResilienceScenario::decl(), NetworkResilienceProblem::decl(), ResilienceScenarioResult::decl(), FlowRadiusValue::decl(), ResilienceEffectiveCapacities::decl(), ResilienceConnectivityFlow::decl(), NetworkResilienceSolution::decl(), NetworkResilienceReport::decl(), VrpAxis::decl(), VrpNodeSpec::decl(), VrpTimeWindow::decl(), VrpDepotSpec::decl(), VrpCustomerSpec::decl(), VrpVehicleTypeSpec::decl(), VrpArcSpec::decl(), VrpUnitsSpec::decl(), VrpTolerancesSpec::decl(), VrpSearchOptions::decl(), VrpProblem::decl(), VrpStop::decl(), VrpRoute::decl(), VrpSolution::decl(), VrpReport::decl(), IndexedKey::decl(), IndexedKeyIndex::decl(), IndexedRegistrationMode::decl(), IndexedVariableKind::decl(), IndexedRegistrationRequest::decl(), IndexedRegistrationReport::decl(), LinearColumnCompletion::decl(), LinearColumnScope::decl(), LinearColumnBatch::decl(), LinearColumnCertificate::decl(), HeartBeatRequest::decl(), HeartBeatSnapshot::decl(), ResponseEnvelope::decl(), LinearColumnState::decl(), LinearColumnOperationKind::decl(), LinearColumnOperation::decl(), LinearLifecycleRequest::decl(), LinearColumnStateEntry::decl(), LinearLifecycleReport::decl(), MasterVariableDomain::decl(), MasterVariableSpec::decl(), MasterCutCoefficient::decl(), MasterCutSense::decl(), MasterCutScope::decl(), MasterCutSpec::decl(), CpAssumptionSpec::decl(), LogicBasedBendersRequest::decl(), LogicBasedBendersReport::decl(), LogicBasedBendersAssumptionBinding::decl(), LogicBasedBendersPlanRequest::decl(), LogicBasedBendersPlanReport::decl(), MasterAssignmentValue::decl(), QueryColumn::decl(), QueryComparison::decl(), QueryScalar::decl(), QueryTrivalent::decl(), QueryFilter::decl(), QueryJoinType::decl(), QueryJoinCardinality::decl(), QuerySourceSpec::decl(), QueryJoinSpec::decl(), QuerySortDirection::decl(), QueryNullsOrder::decl(), QueryOrderSpec::decl(), QueryPageSpec::decl(), QueryPlanLimits::decl(), QueryPlanViolation::decl(), QueryPlanReport::decl(), RelationalQueryPlanSpec::decl(), BendersCutCoefficient::decl(), BendersCutSense::decl(), BendersCutSpec::decl(), BendersMasterReport::decl(), BendersQuadraticTerm::decl(), BendersQuadraticCutSpec::decl(), BendersSubOutcome::decl(), BendersSubReport::decl(), RemoteArtifactReport::decl(), RemoteArtifactRequest::decl(), RemoteCodecOperation::decl(), RemoteFingerprintSpec::decl(), RemoteResumeExpectation::decl(), RemoteCheckpointIdentity::decl(), ConflictExplanationSummary::decl(), ConflictGroupEntry::decl(), CandidateFunnelReport::decl(), EffectivenessProbeSpec::decl(), EffectivenessProbeRow::decl(), EffectivenessProbeRequest::decl(), EffectivenessProbeReport::decl(), CriticalReportRequest::decl(), CriticalReportVerdict::decl(), GroupActivitySpec::decl(), GroupEffectivenessSpec::decl(), GroupSummaryRow::decl(), GroupSummaryRequest::decl(), GroupSummaryReport::decl(), BandProbeSpec::decl(), BandProbeRow::decl(), BandProbeRequest::decl(), BandProbeReport::decl(), InfeasibilityEvidenceSpec::decl(), EvidenceProbeRequest::decl(), EvidenceProbeReport::decl(), ProofSpec::decl(), ProofProbeRequest::decl(), ProofProbeReport::decl(), SnapshotSpec::decl(), GapProbeRequest::decl(), GapProbeReport::decl(), EvaluationSpec::decl(), MessageSpec::decl(), DiagnosticsProbeRequest::decl(), DiagnosticsProbeReport::decl(), AuditFingerprintSpec::decl(), FingerprintProbeRequest::decl(), FingerprintProbeReport::decl(), VerificationSpec::decl(), BlockingSetRequest::decl(), BlockingSetReport::decl(), CorrectionCandidateSpec::decl(), CorrectionMemberRow::decl(), CorrectionProbeRequest::decl(), CorrectionProbeReport::decl(), LocalSensitivitySpec::decl(), SensitivityProbeRequest::decl(), SensitivityProbeReport::decl(), FunnelSortCandidateSpec::decl(), FunnelCandidateRow::decl(), FunnelProbeRequest::decl(), FunnelProbeReport::decl(), EffectivenessEntrySpec::decl(), EffectivenessEntryRow::decl(), EffectivenessRankingRequest::decl(), EffectivenessRankingReport::decl(), PersistenceSortItemSpec::decl(), PersistenceSortRow::decl(), PersistenceSortProbeRequest::decl(), PersistenceSortProbeReport::decl(), PersistenceFieldSpec::decl(), SchemaQueryRow::decl(), PersistenceSchemaRequest::decl(), PersistenceSchemaReport::decl(), PredicateScopeSpec::decl(), PredicateScopeRequest::decl(), PredicateScopeReport::decl(), NativeTerminalProofRequest::decl(), NativeTerminalProofReport::decl(), NativeOutcomeSpec::decl(), NativeLoweringProbeRequest::decl(), NativeLoweringProbeReport::decl(), SolveValueProbeRequest::decl(), SolveValueProbeReport::decl(), SelectionStepSpec::decl(), SelectionObservationSpec::decl(), SelectionProbeRequest::decl(), SelectionProbeReport::decl(), NonStationarySelectSpec::decl(), NonStationaryObservationSpec::decl(), NonStationaryProbeRequest::decl(), NonStationaryProbeReport::decl(), TrainingCaseSpec::decl(), TrainingParameterSpec::decl(), TrainingSplitProbeRequest::decl(), TrainingSplitProbeReport::decl(), AcceptanceDefinitionSpec::decl(), AcceptanceOverrideSpec::decl(), AcceptanceLoadRow::decl(), AcceptanceRecoveryRequest::decl(), AcceptanceRecoveryReport::decl(), HypervolumeMeasurementSpec::decl(), HypervolumeMeasureRow::decl(), HypervolumeProbeRequest::decl(), HypervolumeProbeReport::decl(), OperatorRegistrationSpec::decl(), OperatorRegistrationRow::decl(), OperatorCapabilityRequest::decl(), OperatorCapabilityReport::decl(), ScheduleEventSpec::decl(), ScheduleLogSpec::decl(), ScheduleLogRow::decl(), ScheduleLogRequest::decl(), ScheduleLogReport::decl(), ShortlistEntry::decl(), EffectivenessReport::decl(), EffectivenessRow::decl(), CriticalAnalysisReport::decl(), CriticalBaseline::decl(), CriticalStagePresence::decl(), RelaxationRequest::decl(), RelaxationPolicyInput::decl(), RelaxationCostInput::decl(), RelaxationMember::decl(), RelaxationPlan::decl(), RelaxationReport::decl(), FixedIntegerConfigInput::decl(), FixedIntegerLpReport::decl(), FixedIntegerSensitivityRow::decl(), PerturbationPolicyRequest::decl(), AdaptivePerturbationInput::decl(), AdaptivePerturbationEcho::decl(), PerturbationObservationRow::decl(), PerturbationReportSummary::decl(), PerturbationVerification::decl(), TargetSpec::decl(), TargetEcho::decl(), TargetReportSummary::decl(), TargetVerification::decl(), MultiTargetReportSummary::decl(), MultiTargetResultRow::decl(), BottleneckKindRow::decl(), CriticalityRequest::decl(), CriticalityObservationInput::decl(), CriticalityObservationRow::decl(), CriticalityProfileReport::decl(), HyperHeuristicConfigRequest::decl(), HyperHeuristicConfigReport::decl(), AlgorithmRow::decl(), BudgetRow::decl(), ExecutionRow::decl(), MemoryRow::decl(), LearningRow::decl(), PersistenceRow::decl(), RewardPolicyRequest::decl(), RewardPolicyReport::decl(), TimeNormalizedInput::decl(), ExtremeValueInput::decl(), HypervolumeConfigInput::decl(), HypervolumeRow::decl(), CheckpointRequest::decl(), CheckpointReport::decl(), LinearMappingRequest::decl(), LinearMappingReport::decl(), LbbResumeIdentityRequest::decl(), LbbResumeIdentityReport::decl(), FingerprintInput::decl(), ProvenanceInput::decl(), CancellationInput::decl(), CpCheckpointRequest::decl(), CpCheckpointReport::decl(), BendersMetricsRequest::decl(), BendersMetricsReport::decl(), BendersSnapshotInput::decl(), BendersSnapshotRow::decl(), BendersCutRequest::decl(), BendersCutReport::decl(), BendersCutRow::decl(), SymbolConditionRequest::decl(), SymbolConditionBounds::decl(), SymbolConditionReport::decl(), SymbolConditionSample::decl(), SymbolLinearizationRequest::decl(), SymbolLinearizationReport::decl(), SymbolBinaryzationRow::decl(), SymbolCoreRelation::decl(), SymbolCoreRelationsReport::decl(), SymbolImplyRow::decl(), SymbolInequalityRow::decl(), SymbolInValuesTable::decl(), SymbolCoreRelationsRequest::decl(), HeuristicScheduleRequest::decl(), HeuristicScheduleReport::decl(), HeuristicScheduleSample::decl(), QTableRequest::decl(), QTableReport::decl(), QTransition::decl(), QTransitionOutcome::decl(), QSelector::decl(), QSelectionOutcome::decl(), MasterCutEncodingRequest::decl(), MasterCutEncodingReport::decl(), MasterAssignmentEntry::decl(), MasterNoGoodRowSpec::decl(), AnalysisGroupAggregateRequest::decl(), AnalysisGroupAggregateReport::decl(), AnalysisGroupRow::decl(), PredicateFieldResolutionRequest::decl(), PredicateFieldResolutionReport::decl(), ScalarFunctionRequest::decl(), ScalarFunctionReport::decl(), ScalarFunctionRow::decl(), PolynomialLiftRequest::decl(), PolynomialLiftReport::decl(), LinearTermSpec::decl(), PowerSpec::decl(), BoundedNoGoodRequest::decl(), BoundedNoGoodReport::decl(), BoundedNoGoodRow::decl(), BoundedNoGoodVariableSpec::decl(), BoundedNoGoodValue::decl(), VerifiedConflictRow::decl(), EffectivenessAggregateRequest::decl(), EffectivenessAggregateReport::decl(), AggregateRow::decl(), ObservationSpec::decl(), PerturbationSpec::decl(), FunnelCandidateSpec::decl(), AssumptionBindingRequest::decl(), AssumptionBindingReport::decl(), AssumptionRow::decl(), AssumptionRow2::decl(), AssumptionSpec::decl(), AssumptionMappingSpec::decl(), TierClassificationRequest::decl(), TierClassificationReport::decl(), CandidateSpec::decl(), TierRow::decl(), PolarityRequest::decl(), PolarityReport::decl(), BinaryVariableSpec::decl(), IntegerVariableSpec::decl(), SparseTerm::decl(), RowClosedForm::decl(), FamilyRowForms::decl(), MonomialSpec::decl(), MonomialReport::decl(), MaterializeRequest::decl(), BackendFeaturesRequest::decl(), CanonicalMonomialRequest::decl(), ExpressionKeyRequest::decl(), EncodingLabelRequest::decl(), ActivityBandRequest::decl(), EvidenceSpecRequest::decl(), ActivityReportRequest::decl(), AuxiliaryDomainRequest::decl(), BindingAssignmentRequest::decl(), SourceIdentityRequest::decl(), CutValidityRequest::decl(), AssignmentCompositionRequest::decl(), DumpOptionsSpec::decl(), NativeWriteScopeRequest::decl(), WriterRegistryRequest::decl(), ShortlistRequest::decl(), LazyInitRequest::decl(), LazyInitReport::decl(), DumpBatchRequest::decl(), DumpBatchReport::decl(), DumpFileRow::decl(), QuadraticElasticRequest::decl(), QuadraticElasticReport::decl(), StableValueRequest::decl(), StableValueReport::decl(), StableValueEntry::decl(), MasterAssignmentRequest::decl(), MasterAssignmentReport::decl(), MasterAssignedVariableSpec::decl(), MasterAssignedValue::decl(), SparseTriadRequest::decl(), SparseTriadReport::decl(), SparseEntrySpec::decl(), SparseBoundsSpec::decl(), SparseBoundsRow::decl(), MatrixRowsRequest::decl(), MatrixRowsReport::decl(), MatrixRowSpec::decl(), MatrixEntrySpec::decl(), PolynomialProbeRequest::decl(), PolynomialProbeReport::decl(), PolynomialTermSpec::decl(), LinearTermRow::decl(), ModelViewRequest::decl(), ModelViewReport::decl(), AnalysisOptionsRequest::decl(), AnalysisOptionsReport::decl(), CapabilityMatrixRequest::decl(), CapabilityRow::decl(), CapabilityMatrixReport::decl(), CpConstraintLevelSpec::decl(), CpSupportMergeRequest::decl(), CpFeatureRow::decl(), CpSupportMergeReport::decl(), SubproblemValueSpec::decl(), SubproblemResultRequest::decl(), SubproblemResultReport::decl(), OracleBindingLiteral::decl(), OracleAssignmentEntry::decl(), NoGoodOracleRequest::decl(), OracleCutView::decl(), NoGoodOracleReport::decl(), CpVariableSpec::decl(), CpConstraintSpec::decl(), CpGroupSpec::decl(), CpModelBuildRequest::decl(), CpModelBuildReport::decl(), SnapshotValueSpec::decl(), SnapshotGateRequest::decl(), SnapshotGateReport::decl(), ConstraintShapeSpec::decl(), ConstraintCanonRequest::decl(), ConstraintCanonView::decl(), ConstraintCanonReport::decl(), AssignmentValueSpec::decl(), AssignmentEvalRequest::decl(), IntervalValueView::decl(), AssignmentEvalReport::decl(), ExpressionTermSpec::decl(), ExpressionDomainSpec::decl(), ExpressionBoundsRequest::decl(), ExpressionTermView::decl(), ExpressionBoundsReport::decl(), DomainProbeSpec::decl(), DomainProbeRequest::decl(), DomainView::decl(), DomainProbeReport::decl(), FingerprintSpec::decl(), IdentityWrapRequest::decl(), IdentityWrapView::decl(), IdentityWrapReport::decl(), ContainerTermSpec::decl(), ContainerProbeRequest::decl(), ContainerView::decl(), ContainerProbeReport::decl(), ActivationVariableSpec::decl(), ActivationSourceSpec::decl(), ActivationOpSpec::decl(), ActivationRecordView::decl(), ActivationSetRequest::decl(), ActivationSetReport::decl(), TierOverrideSpec::decl(), TierSelectionRequest::decl(), TierSelectionReport::decl(), EvidenceSourceVariableSpec::decl(), EvidenceSourceSpec::decl(), EvidenceMemberSpec::decl(), EvidenceRemapRequest::decl(), EvidenceSourceView::decl(), EvidenceRemapReport::decl(), DominancePairSpec::decl(), DominanceRequest::decl(), DominancePairView::decl(), DominanceReport::decl(), DispersionRequest::decl(), CrowdingPointView::decl(), DispersionFrontView::decl(), DispersionReport::decl(), ReferencePairSpec::decl(), ReferencePairView::decl(), ReferencePointsRequest::decl(), ReferencePointsReport::decl(), ArchiveRequest::decl(), ArchiveReport::decl(), RankingAssessmentSpec::decl(), ConstraintRankingRequest::decl(), RankingEntryView::decl(), ConstraintRankingReport::decl(), PriorityRequest::decl(), PriorityView::decl(), PriorityReport::decl(), PopulationConfigRequest::decl(), PopulationConfigReport::decl(), BoundaryRequest::decl(), BoundaryReport::decl(), AcceptanceRequest::decl(), AcceptanceReport::decl(), RankingItemRow::decl(), RankingProbeRequest::decl(), RankingProbeReport::decl(), TopologyRequest::decl(), TopologyReport::decl(), MigrationRequest::decl(), MigrationReport::decl(), ArchiveStepRow::decl(), ArchiveProbeRequest::decl(), ArchiveProbeReport::decl(), PiecewisePointSpec::decl(), PiecewiseRequest::decl(), PiecewiseReport::decl(), DiversityRequest::decl(), DiversityReport::decl(), SolutionStatusRequest::decl(), SolutionStatusReport::decl(), FunnelProbeSpec::decl(), FunnelRow::decl(), FunnelRequest::decl(), FunnelReport::decl(), ShortlistReport::decl(), CandidateEntrySpec::decl(), WriterRegistryReport::decl(), WriterSpec::decl(), DestinationRow::decl(), NativeWriteScopeReport::decl(), WriteRecordSpec::decl(), NativeStructureSpec::decl(), OutcomeSpec::decl(), OutcomePredicates::decl(), DumpOptionsReport::decl(), DumpOptionsReportDefaults::decl(), FixedVariableSpec::decl(),  AssignmentCompositionReport::decl(), AssignStep::decl(), AssignLiteral::decl(), AssignOutcome::decl(), PathOutcome::decl(), CutValidityReport::decl(), CutValidityOutcome::decl(), CutValiditySpec::decl(), SourceIdentityReport::decl(), SourceIdentity::decl(), SourceSpec::decl(), BindingAssignmentReport::decl(), BindingLiteral::decl(), AuxiliaryDomainReport::decl(), AuxiliarySpec::decl(), AuxiliaryOutcome::decl(), ActivityReportVerdict::decl(), BoundRecordSpec::decl(), ConstraintRecordSpec::decl(), GroupSummarySpec::decl(), StatusCountSpec::decl(), EvidenceReport::decl(), EvidenceSpec::decl(), EvidenceRow::decl(), ActivityBandReport::decl(), ActivityBandRow::decl(), EncodingLabelReport::decl(), EncodingAssignmentRow::decl(), EncodingTerm::decl(), ExpressionKeyReport::decl(), KeyTerm::decl(), CanonicalMonomialReport::decl(), PowerEntry::decl(), PowerRow::decl(), BackendFeaturesReport::decl(), CompileTimeSnapshot::decl(), MaterializeReport::decl(), StructureSpec::decl(), StructureTrace::decl(), PredicateFieldProbeReport::decl(), PredicateFieldMapping::decl(), PredicateFieldFilterRequest::decl(), PredicateFieldFilterReport::decl(), FilterConditionSpec::decl(), FilterValueSpec::decl(), EqFilterRow::decl(), EqOrInRow::decl(), FieldFilterRow::decl(), ModelConfigurationRequest::decl(), ModelConfigurationReport::decl(), BuildingStatusSpec::decl(), BuildingStatusRow::decl(), ModelConfigurationDefaults::decl(), CacheKeyIdentityRequest::decl(), CacheKeyIdentityReport::decl(), CacheOperationSpec::decl(), CacheOperationRow::decl(), ValueKeySpec::decl(), RangeKeySpec::decl(), CacheKeyAddressRequest::decl(), CacheKeyAddressReport::decl(), EqualValueGroup::decl(), FlattenCacheRequest::decl(), FlattenCacheReport::decl(), FlattenCacheOperation::decl(), FlattenCacheRow::decl(), FlattenSymbolSpec::decl(), UpdateAssignmentRequest::decl(), UpdateAssignmentReport::decl(), AssignmentGroup::decl(), UpdateAssignmentSpec::decl(), AssignmentRow::decl(), UpdateAssignmentValue::decl(), AssignmentExpressionSpec::decl(), BooleanFoldRequest::decl(), BooleanFoldReport::decl(), ScopeStep::decl(), ScopeStepRow::decl(), ScopeValue::decl(), UnsupportedPredicateRequest::decl(), UnsupportedPredicateReport::decl(), PolicyRow::decl(), PolicyAlias::decl(), AnnotationSpec::decl(), RepositoryCallRequest::decl(), RepositoryCallReport::decl(), RepositoryCallRow::decl(), FunctionUsageRequest::decl(), FunctionUsageReport::decl(), UsageFlags::decl(), UsageRow::decl(), InputDomainRequest::decl(), InputDomainReport::decl(), DomainSpec::decl(), DomainPairSpec::decl(), DomainRow::decl(), DomainPairRow::decl(), FloatSpec::decl(), FingerprintRow::decl(), WriteFingerprintRow::decl(), FingerprintCase::decl(), NativeWriteRequest::decl(), NativeWriteReport::decl(), BindingSpec::decl(), BindingRow::decl(), ProofPair::decl(), ProofRow::decl(), ElasticModelRequest::decl(), ElasticModelReport::decl(), ModelVariableSpec::decl(), ModelRowSpec::decl(), TransformRow::decl(), SlackRow::decl(), KindPair::decl(), CombineStatusRequest::decl(), CombineStatusReport::decl(), CombineCell::decl(), NumberTextRequest::decl(), NumberTextReport::decl(), NumberTextRow::decl(), TextFloatSpec::decl(), BigMPolicyRequest::decl(), BigMPolicyReport::decl(), BigMPolynomialSpec::decl(), BigMProbeRow::decl(), BigMIndicatorRow::decl(), ConditionalSemanticsRequest::decl(), ConditionalSemanticsReport::decl(), ConditionPolynomialSpec::decl(), ConditionBoundsSpec::decl(), ClassificationRow::decl(), RelationSemanticsRow::decl(), BoundsProbeRow::decl(), EntryShapeRow::decl(), AliasRow::decl(), SearchBudgetRequest::decl(), SearchBudgetReport::decl(), SliceVerdict::decl(), RandomVerdict::decl(), PredicateSchemaRequest::decl(), PredicateSchemaReport::decl(), SchemaFieldSpec::decl(), ResolveRow::decl(), SameFieldRow::decl(), SortByRequest::decl(), SortByReport::decl(), SortItemSpec::decl(), SortedItemRow::decl(), SupportRow::decl(), VariableKindDomainRequest::decl(), VariableKindDomainReport::decl(), VariableKindRow::decl(), CriticalAssemblyRequest::decl(), CriticalAssemblyReport::decl(), AssemblyCase::decl(), AssemblyRow::decl(), FlattenIdentityRequest::decl(), FlattenIdentityReport::decl(), MasterCutRequest::decl(), MasterCutReport::decl(), MasterCutTerm::decl(), MasterCutTermRow::decl(), FieldResolutionRequest::decl(), FieldResolutionReport::decl(), BooleanScopeRequest::decl(), BooleanScopeReport::decl(), RemoteArtifactKind::decl(), FieldFilterSpec::decl(), FieldFilterEntry::decl(), FieldFilterReport::decl(), AssignmentKind::decl(), AssignmentValue::decl(), AssignmentSpec::decl(), AssignmentEntry::decl(), AssignmentReport::decl(), NullsSupportLevel::decl(), SortItemVerdict::decl(), SortPlanSpec::decl(), SortPlanReport::decl(), RepositoryQuerySpec::decl(), PipelineGroupSpec::decl(), PipelineStepSpec::decl(), PipelineStepVerdict::decl(), PipelineGroupConflict::decl(), PipelinePlanReport::decl(), QuadraticOrchestration::decl(), QuadraticOrchestrationOptions::decl(), QuadraticOrchestrationReport::decl(), AnalysisFunnelOptions::decl(), MipLoweringAnalysis::decl(),
        IntegerSolutionHint::decl(), ConstraintProgrammingOptions::decl(),
        NativeBackend::decl(), SerialFallbackPolicy::decl(), SerialLinearOptions::decl(),
        ParallelCombinatorialMode::decl(), ParallelLinearOptions::decl(),
        FixedRecourseStage::decl(), StagedVariable::decl(), FixedRecourseOptions::decl(), FixedRecourseStop::decl(), FixedRecourseSummary::decl(),
        ColumnGenerationBlock::decl(), ColumnGenerationOptions::decl(), ColumnGenerationStop::decl(), ColumnGenerationSummary::decl(),
        QuantityUnit::decl(), PhysicalQuantity::decl(), Bpp1dItem::decl(), Bpp1dBinType::decl(), Bpp1dObjective::decl(), Bpp1dProblem::decl(),
        BppLoadingItem::decl(), BppLoadingPosition::decl(), BppLoadingAssignment::decl(), BppLoadingConflict::decl(), BppLoadingLayout::decl(),
        BppLoadingLayoutGroup::decl(), BppLoadingInterval::decl(), BppLoadingRectangle::decl(), BppLoadingProblem::decl(), BppLoadingSolution::decl(), BppLoadingResult::decl(),
        BppLoadingLayoutCost::decl(), BppLoadingObjective::decl(), BppLoadingOptions::decl(), BppLoadingApplicationEnvelope::decl(),
        BppLoadingResource::decl(), BppLoadingResourceResult::decl(),
        BppLoadingSourceFinishingOptions::decl(), BppLoadingCertificationEvidence::decl(), BppLoadingBendersOptions::decl(), BppLoadingBendersEnvelope::decl(),
        BppLoadingPartition::decl(), BppLoadingPartitionOptions::decl(), BppLoadingPartitionDecisionRole::decl(), BppLoadingPartitionDecision::decl(),
        BppLoadingPartitionColumnInput::decl(), BppLoadingPartitionColumn::decl(), BppLoadingPartitionMasterOptions::decl(), BppLoadingPartitionGlobalBinding::decl(), BppLoadingPartitionPricingResult::decl(),
        BppLoadingPartitionExecutionOptions::decl(), BppLoadingPartitionObservation::decl(), BppLoadingPartitionCall::decl(), BppLoadingPartitionEnvelope::decl(),
        BppLoadingPartitionIteration::decl(), BppLoadingPartitionSummary::decl(),
        Geometry2dPoint::decl(), Geometry2dShape::decl(), Geometry2dBox::decl(), Geometry2dPlacement::decl(), Geometry2dSheet::decl(),
        Geometry2dShapeMetrics::decl(), Geometry2dBoxMetrics::decl(), Geometry2dShapeCandidate::decl(),
        Bpp1dPattern::decl(), Bpp1dUsage::decl(), Bpp1dGenerationOptions::decl(), Bpp1dPatterns::decl(), Bpp1dPlacement::decl(), Bpp1dPlan::decl(),
        Bpp2dItem::decl(), Bpp2dSheetType::decl(), Bpp2dDemand::decl(), Bpp2dLayer::decl(), Bpp2dObjective::decl(), Bpp2dProblem::decl(),
        Bpp2dGeneratorKind::decl(), Bpp2dOptions::decl(), Bpp2dStageKind::decl(), Bpp2dExecutionStop::decl(), Bpp2dPricedColumn::decl(), Bpp2dPricingIteration::decl(),
        Bpp2dStageSummary::decl(), Bpp2dExecutionSummary::decl(), Bpp2dUsage::decl(), Bpp2dProduction::decl(),
        Bpp2dRenderPlacement::decl(), Bpp2dRenderSheet::decl(), Bpp2dPackedPattern::decl(), Bpp2dPacking::decl(),
        Bpp3dOrientation::decl(), Bpp3dAxis::decl(), Bpp3dShape::decl(), Bpp3dPoint::decl(), Bpp3dItem::decl(), Bpp3dBin::decl(), Bpp3dPlacement::decl(), Bpp3dLayer::decl(), Bpp3dProblem::decl(), Bpp3dRadiusPrototype::decl(), Bpp3dRadiusCandidates::decl(), Bpp3dPackageAttribute::decl(), Bpp3dRadiusComponent::decl(), Bpp3dRadiusObjective::decl(), Bpp3dRadiusWeightFunction::decl(),
        Bpp3dAssignment::decl(), Bpp3dPackedBin::decl(), Bpp3dPacking::decl(), Bpp3dGeneratorKind::decl(), Bpp3dOptions::decl(),
        Bpp3dExecutionStageKind::decl(), Bpp3dExecutionStageSummary::decl(), Bpp3dExecutionSummary::decl(),
        GanttTimeRange::decl(), GanttActivityEndpoint::decl(), GanttActivityPresence::decl(), GanttActivityResourceUse::decl(),
        GanttActivityMode::decl(), GanttActivity::decl(), GanttActivityCapacity::decl(), GanttActivityResource::decl(),
        GanttActivityDependency::decl(), GanttActivityProblem::decl(), GanttActivitySourceSummary::decl(), GanttActivityOptions::decl(),
        GanttActivitySelection::decl(), GanttActivityPlan::decl(), GanttActivityReservation::decl(),
        GanttActivityPlacement::decl(), GanttActivityReplayReport::decl(), GanttActivitySchedule::decl(), GanttActivityExecutionReport::decl(),
        Bpp1dCsvOptions::decl(), Bpp1dFreeInterval::decl(), Bpp1dKpi::decl(), Bpp1dPacking::decl(), Bpp1dRenderItem::decl(), Bpp1dRenderBin::decl(), Bpp1dRenderSchema::decl(), Bpp1dAlgorithm::decl(), Bpp1dOptions::decl(), Bpp1dSummary::decl(),
        Csp1dWidthRange::decl(), Csp1dProduct::decl(), Csp1dCostar::decl(), Csp1dMaterial::decl(), Csp1dMachine::decl(), Csp1dDemandMode::decl(), Csp1dDemand::decl(), Csp1dProblem::decl(),
        Csp1dSlice::decl(), Csp1dPattern::decl(), Csp1dUsage::decl(), Csp1dGeneratorKind::decl(), Csp1dGenerationOptions::decl(), Csp1dPatterns::decl(), Csp1dProduction::decl(),
        Csp1dStageKind::decl(), Csp1dProductionReportSummary::decl(),
        Csp2dProduct::decl(), Csp2dCuttingProcess::decl(), Csp2dMaterialShape::decl(), Csp2dGenerationProduct::decl(), Csp2dMaterial::decl(),
        Csp2dDemand::decl(), Csp2dRegion::decl(), Csp2dCutOrientation::decl(), Csp2dCutNode::decl(), Csp2dCutTree::decl(),
        Csp2dColumn::decl(), Csp2dObjective::decl(), Csp2dProblem::decl(),
        Csp2dGeneratorKind::decl(), Csp2dOptions::decl(), Csp2dStageKind::decl(), Csp2dExecutionStop::decl(),
        Csp2dPricedColumn::decl(), Csp2dPricingIteration::decl(), Csp2dStageSummary::decl(), Csp2dExecutionSummary::decl(), Csp2dUsage::decl(), Csp2dProduction::decl(),
        Csp1dAlgorithm::decl(), Csp1dOptions::decl(), Csp1dBusinessStatus::decl(), Csp1dExecutionStop::decl(), Csp1dExecutionSummary::decl(),
        HostCapabilities::decl(), JobStatus::decl(), JobView::decl(), ReportSummary::decl()]
        .into_iter().map(|decl| format!("export {decl}\n")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_protocol_fields() {
        assert!(serde_json::from_str::<ModelArtifact>(r#"{"modelJson":"{}","fingerprint":"x","extra":1}"#).is_err());
    }

    #[test]
    fn declarations_are_generated_from_rust() {
        assert!(typescript_declarations().contains("protocolVersion: number"));
        assert!(typescript_declarations().contains("modelKind?: ModelKind"));
        assert!(typescript_declarations().contains("type ConstraintOptions"));
        assert!(typescript_declarations().contains("nearlyActiveTolerance?: number"));
        assert!(typescript_declarations().contains("SATISFIED_WITHOUT_SLACK_METRIC"));
        assert!(typescript_declarations().contains("type Csp1dProductionReportSummary"));
        assert!(typescript_declarations().contains("restrictedOptimal: boolean"));
        assert!(typescript_declarations().contains("bpp2dOptions?: Bpp2dOptions"));
        assert!(typescript_declarations().contains("csp2dOptions?: Csp2dOptions"));
    }

    #[test]
    fn declarations_cover_every_referenced_type() {
        // 子串抽查无法阻止"被引用但未注册"的回归（Bpp3dShape 与 SolverTuningOptions 都这样漏过）；
        // 这里改成集合覆盖断言：生成的声明文本里出现的每个类型标识符，要么被声明，要么是内置类型。
        // Substring spot checks cannot stop "referenced but unregistered" regressions (both Bpp3dShape and
        // SolverTuningOptions slipped through that way), so this asserts set coverage over type identifiers.
        let text = typescript_declarations();
        let characters: Vec<char> = text.chars().collect();
        let mut cleaned = String::with_capacity(text.len());
        let mut index = 0;
        while index < characters.len() {
            let current = characters[index];
            let next = characters.get(index + 1).copied();
            if current == '/' && next == Some('/') {
                let newline = char::from(10);
                while index < characters.len() && characters[index] != newline { index += 1; }
                continue;
            }
            if current == '/' && next == Some('*') {
                index += 2;
                while index + 1 < characters.len() && !(characters[index] == '*' && characters[index + 1] == '/') { index += 1; }
                index += 2;
                continue;
            }
            if current == '"' || current == char::from(39) || current == '`' {
                let quote = current; index += 1;
                while index < characters.len() && characters[index] != quote { index += 1; }
                index += 1;
                cleaned.push(' ');
                continue;
            }
            cleaned.push(current); index += 1;
        }
        let mut declared = std::collections::HashSet::new();
        for line in cleaned.lines() {
            let line = line.trim();
            for prefix in ["export type ", "export interface ", "export class "] {
                if let Some(rest) = line.strip_prefix(prefix) {
                    let name: String = rest.chars().take_while(|value| value.is_alphanumeric() || *value == '_').collect();
                    if !name.is_empty() { declared.insert(name); }
                }
            }
        }
        let builtins = ["String", "Boolean", "Number", "BigInt", "Object", "Array", "ReadonlyArray", "Record", "Map",
            "ReadonlyMap", "Set", "ReadonlySet", "Promise", "Date", "Uint8Array", "Int8Array", "Float64Array", "Float32Array",
            "Uint32Array", "Int32Array", "Uint16Array", "Int16Array", "ArrayBuffer", "Partial", "Pick", "Omit", "Readonly",
            "NonNullable", "Exclude", "Extract", "Iterable", "True", "False"];
        let mut unknown = std::collections::BTreeSet::new();
        for word in cleaned.split(|value: char| !(value.is_alphanumeric() || value == '_')) {
            let mut characters = word.chars();
            let Some(first) = characters.next() else { continue };
            if !first.is_uppercase() || word.chars().any(|value| !value.is_ascii()) { continue; }
            if declared.contains(word) || builtins.contains(&word) { continue; }
            unknown.insert(word.to_string());
        }
        assert!(unknown.is_empty(), "declarations reference unregistered types: {unknown:?}");
    }

    #[test]
    fn report_summary_only_claims_optimality_with_a_verified_proof() {
        use ospf_rust_core::solver::report::{ProofKind, ProofStatus};
        use ospf_rust_core::solver::{ProblemStatus, SolveProof, SolveReport, SolveSolution, TerminationReason};
        let text = |proof: Option<SolveProof<f64>>, reason: TerminationReason| {
            let mut builder = SolveReport::<f64>::builder(ProblemStatus::Feasible, reason)
                .solution(SolveSolution::vector(vec![1.0]));
            if let Some(proof) = proof { builder = builder.proof(proof); }
            serde_json::to_string(&builder.build().unwrap()).unwrap()
        };
        // 没有证明 / No proof at all.
        let plain = summarize_report(&text(None, TerminationReason::GapLimit)).unwrap();
        assert!(!plain.has_optimality_proof);
        assert_eq!(plain.termination_reason, "GAP_LIMIT");
        // 已验证的最优性证明 / A verified optimality proof.
        let certified = summarize_report(&text(Some(SolveProof::optimality()), TerminationReason::Completed)).unwrap();
        assert!(certified.has_optimality_proof);
        // 仅声称未验证的证明不算数 / A merely claimed, unverified proof does not count.
        let mut claimed = SolveProof::optimality();
        claimed.status = ProofStatus::Claimed;
        assert!(!summarize_report(&text(Some(claimed), TerminationReason::Completed)).unwrap().has_optimality_proof);
        // 不可行证明不是最优性证明；core 要求不可行证明必须配不可行状态。
        // An infeasibility proof is not an optimality proof, and core requires it to pair with an infeasible status.
        let mut infeasibility = SolveProof::optimality();
        infeasibility.kind = ProofKind::Infeasibility;
        let text_infeasible = serde_json::to_string(&SolveReport::<f64>::builder(ProblemStatus::Infeasible, TerminationReason::Completed)
            .proof(infeasibility).build().unwrap()).unwrap();
        assert!(!summarize_report(&text_infeasible).unwrap().has_optimality_proof);
    }

    #[test]
    fn every_typescript_type_in_the_source_is_registered() {
        // 前两次漏注册（SolverTuningOptions、Bpp3dShape）和第三次（Bpp1dRender*）都说明：只断言"生成文本内部自洽"
        // 抓不到"压根没注册"。这里直接扫源码里的 #[derive(.. TS ..)]，与注册表集合比对。
        // Earlier misses (SolverTuningOptions, Bpp3dShape, then Bpp1dRender*) show that asserting internal consistency
        // of the generated text cannot catch a type that is never registered at all. Scan the sources instead.
        fn visit(dir: &std::path::Path, found: &mut std::collections::BTreeSet<String>) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() { visit(&path, found); continue; }
                if path.extension().and_then(|value| value.to_str()) != Some("rs") { continue; }
                let Ok(text) = std::fs::read_to_string(&path) else { continue };
                let mut derived = false;
                for line in text.lines() {
                    let line = line.trim();
                    if line.starts_with("#[derive(") { derived = derived || line.contains("TS"); continue; }
                    if !derived { continue; }
                    // derive 与声明之间还可能有 #[serde(..)]、#[ts(..)] 等属性，不能在此丢标记。
                    // Further attributes such as #[serde(..)] or #[ts(..)] may sit between the derive and the
                    // declaration, so intermediate attributes must not clear the flag.
                    if line.starts_with("#[") || line.starts_with("///") || line.is_empty() { continue; }
                    if line.starts_with("pub struct ") || line.starts_with("pub enum ") {
                        let name: String = line.split_whitespace().nth(2).unwrap_or_default()
                            .trim_end_matches('{').trim_end_matches(';').trim_end_matches('<').to_string();
                        if !name.is_empty() { found.insert(name); }
                    }
                    derived = false;
                }
            }
        }
        let mut found = std::collections::BTreeSet::new();
        visit(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut found);
        assert!(!found.is_empty(), "no TypeScript-derived types were found; the scan itself broke");
        let text = typescript_declarations();
        let missing: Vec<&String> = found.iter().filter(|name| !text.contains(&format!("export type {name}"))).collect();
        assert!(missing.is_empty(), "types derived with TS but never registered: {missing:?}");
    }

    #[test]
    fn model_kind_is_additive_but_unknown_kinds_are_rejected() {
        let json = r#"{"protocolVersion":1,"solver":"scip","model":{"modelJson":"{}","fingerprint":"x"},"timeLimitMs":null}"#;
        let request: SolveRequest = serde_json::from_str(json).unwrap();
        assert!(request.model_kind.is_none());
        let mut input: serde_json::Value = serde_json::from_str(json).unwrap();
        input["modelKind"] = serde_json::json!("quadratic");
        assert!(matches!(serde_json::from_value::<SolveRequest>(input.clone()).unwrap().model_kind, Some(ModelKind::Quadratic)));
        input["modelKind"] = serde_json::json!("cp");
        assert!(matches!(serde_json::from_value::<SolveRequest>(input.clone()).unwrap().model_kind, Some(ModelKind::Cp)));
        input["modelKind"] = serde_json::json!("bpp2d");
        assert!(matches!(serde_json::from_value::<SolveRequest>(input.clone()).unwrap().model_kind, Some(ModelKind::Bpp2d)));
        input["modelKind"] = serde_json::json!("csp2d");
        assert!(matches!(serde_json::from_value::<SolveRequest>(input.clone()).unwrap().model_kind, Some(ModelKind::Csp2d)));
        input["modelKind"] = serde_json::json!("unknown");
        assert!(serde_json::from_value::<SolveRequest>(input).is_err());
    }

    fn example() -> SerializedLinearModel {
        use ospf_rust_framework::solver::remote::{SerializedVariable, SerializedConstraint, SerializedConstraintCell};
        let mut input = SerializedLinearModel::empty("bridge-roundtrip");
        input.variables.push(SerializedVariable {
            stable_id: "x".into(), index: 0, name: "x".into(),
            lower_bound: 0.0, upper_bound: f64::INFINITY,
            variable_type: SerializedVariableType::Continuous,
        });
        input.constraints.push(SerializedConstraint {
            stable_id: "limit".into(), name: "limit".into(), rhs: 2.0,
            sign: SerializedConstraintSign::Equal,
            cells: vec![SerializedConstraintCell { row_index: 0, col_index: 0, coefficient: 1.0 }],
        });
        input
    }

    #[test]
    fn existing_codec_and_fingerprint_roundtrip() {
        let model = compile_linear(&example()).unwrap();
        let artifact = encode_model(&model).unwrap();
        assert!(artifact.model_json.contains("Infinity"));
        let decoded = decode_model(&artifact).unwrap();
        assert_eq!(model.basic.b, decoded.basic.b);
        assert_eq!(model.basic.ub, decoded.basic.ub);
        assert_eq!(artifact.fingerprint, encode_model(&decoded).unwrap().fingerprint);
    }

    #[test]
    fn linear_codec_rejects_lost_metadata_and_solver_variable_types() {
        use ospf_rust_core::variable::VariableType;

        let mut input = example();
        input.variables[0].variable_type = SerializedVariableType::Integer;
        input.variables[0].upper_bound = 2.0;
        let original = compile_linear(&input).unwrap();
        let restored = decode_model(&encode_model(&original).unwrap()).unwrap();
        assert_eq!(restored.basic.var_types, original.basic.var_types);

        let mut metadata = original.clone();
        metadata.basic.constraint_lazy_flags[0] = true;
        assert_eq!(encode_model(&metadata).unwrap_err().code, "Unsupported");
        let compiled = encode_compiled_model(&metadata).unwrap();
        assert_eq!(decode_compiled_model(&compiled).unwrap().basic.constraint_lazy_flags, metadata.basic.constraint_lazy_flags);

        let mut solver_type = original;
        solver_type.basic.var_types[0] = VariableType::Ternary;
        let restored = compile_linear(&serialize_linear_model(&solver_type)).unwrap();
        assert_ne!(restored.basic.var_types, solver_type.basic.var_types);
        assert_eq!(encode_model(&solver_type).unwrap_err().code, "Unsupported");
        let compiled = encode_compiled_model(&solver_type).unwrap();
        assert_eq!(decode_compiled_model(&compiled).unwrap().basic.var_types, solver_type.basic.var_types);
    }

    #[test]
    fn tampering_is_rejected() {
        let mut artifact = encode_model(&compile_linear(&example()).unwrap()).unwrap();
        artifact.fingerprint = "tampered".into();
        assert_eq!(decode_model(&artifact).unwrap_err().code, "FingerprintMismatch");
    }

    #[test]
    fn shared_codec_preserves_infinite_bounds_and_rejects_nan() {
        let mut input = example();
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("Infinity"));
        let decoded: SerializedLinearModel = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.variables[0].upper_bound, f64::INFINITY);
        input.variables[0].upper_bound = f64::NAN;
        assert!(serde_json::to_string(&input).is_err());
    }

    #[test]
    fn shared_variable_range_validation_preserves_domains() {
        let continuous = SerializedVariableType::Continuous;
        let integer = SerializedVariableType::Integer;
        let binary = SerializedVariableType::Binary;
        assert_eq!(linear_variable_range(continuous, f64::NEG_INFINITY, f64::INFINITY).unwrap(), VariableRange::new(None, None));
        assert_eq!(linear_variable_range(integer, -9_007_199_254_740_991.0, 4.0).unwrap(), VariableRange::bounded(-9_007_199_254_740_991.0, 4.0));
        assert_eq!(linear_variable_range(binary, 1.0, 1.0).unwrap(), VariableRange::fixed(1.0));
        for (kind, lower, upper) in [
            (continuous, f64::NAN, 1.0), (continuous, 2.0, 1.0),
            (continuous, f64::INFINITY, f64::INFINITY), (continuous, f64::NEG_INFINITY, f64::NEG_INFINITY),
            (integer, 0.5, 2.0), (integer, 0.0, 9_007_199_254_740_992.0),
            (binary, 0.0, 2.0), (binary, f64::NEG_INFINITY, 1.0),
        ] {
            assert_eq!(linear_variable_range(kind, lower, upper).unwrap_err().code, "InvalidInput");
        }
        assert_eq!(linear_variable_range(SerializedVariableType::Unknown, 0.0, 1.0).unwrap_err().code, "Unsupported");
    }

    #[test]
    fn invalid_shapes_unknown_types_and_precision_are_rejected() {
        let mut input = example();
        input.variables[0].index = 4;
        assert!(compile_linear(&input).is_err());
        input.variables[0].index = 0;
        input.variables[0].variable_type = SerializedVariableType::Unknown;
        assert_eq!(compile_linear(&input).unwrap_err().code, "Unsupported");
        input.variables[0].variable_type = SerializedVariableType::Integer;
        input.variables[0].upper_bound = 9_007_199_254_740_992.0;
        assert!(compile_linear(&input).is_err());
        input.variables[0].upper_bound = 10.0;
        input.constraints[0].cells[0].col_index = 7;
        assert!(compile_linear(&input).is_err());
    }
}

//! BPP2D 应用层 / BPP2D application layer
//!
//! 映射 Kotlin `bpp2d-application`：列生成编排、执行器、应用服务、尾板优化与 CSV 数据集。
//! Maps the Kotlin `bpp2d-application` submodule: column-generation orchestration, executors, the application
//! service, tail sheet optimisation and CSV datasets.

pub mod algorithm;
pub mod column_generation;
pub mod config;
pub mod csv_dataset_loader;
pub mod report_solver;
pub mod service;
pub mod state;
pub mod tail_sheet_optimizer;

pub use algorithm::{
    ColumnGenerationAlgorithm, ColumnGenerationFailureAnalyzer, ColumnGenerationFinalSolver,
    ColumnGenerationHeartbeat, ColumnGenerationRmpSolver, deduplicate_layers, failure_of,
};
pub use config::ColumnGenerationConfig;
pub use column_generation::{Bpp2dColumnGenerationConfig, Bpp2dColumnGenerationInput, Bpp2dColumnGenerationResult, Bpp2dColumnGenerationStage, Bpp2dColumnGenerationStop, Bpp2dPricingIteration, solve_bpp2d_column_generation};
pub use csv_dataset_loader::{CsvDatasetLoader, CsvDatasetSchema, CsvItemRecord, CsvShapeType};
pub use report_solver::{Bpp2dExecutionOptions, Bpp2dMasterResult, Bpp2dNativeBackend, Bpp2dStageReport};
pub use service::{
    ColumnGenerationApplicationService, ColumnGenerationStandardExecutorConfig,
    ColumnGenerationStandardExecutors, ColumnGenerationPackingAnalyzer,
};
pub use state::{
    Bpp2dLpTerminalStatus, ColumnGenerationFailure, ColumnGenerationFailureStage,
    ColumnGenerationFinalResult, ColumnGenerationInput, ColumnGenerationLpResult,
    ColumnGenerationResult, ColumnGenerationState, require_pricing_ready,
};
pub use tail_sheet_optimizer::{TailSheetEvaluation, TailSheetOptimizer, residual_demands};

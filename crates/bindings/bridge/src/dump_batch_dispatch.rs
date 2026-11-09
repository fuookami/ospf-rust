//! 续挖第 57 单：批量导出的**分派与并发语义**——`concurrent` 只改调度、`OPM` 完全忽略转储选项、
//! 失败不回滚且消息带路径（`model/intermediate/lp_export.rs`）
//! Continuation, delivery 57: batch export's **dispatch and concurrency semantics** — `concurrent` only changes scheduling,
//! `OPM` ignores the dump options entirely, and a failure rolls nothing back while the message carries the path
//! (`model/intermediate/lp_export.rs`).
//!
//! 第 52 单量过 `DumpOptions` 本身；这一单量**它怎么被用**：`dump_lp_batch` / `dump_opm_batch` / `dump_batch` /
//! `LPExportableModel`。七条实测（全部用探针模型 + 临时目录，零求解器）：
//!
//! 1. **`concurrent` 只改调度，不改结果**（`concurrent_matches_serial()`）：串行与并发写出的文件**逐字节相同**。
//! 2. **单条目永远走串行**（`a_single_entry_is_always_serial()`）：代码里显式判断 `jobs.len() <= 1`。
//! 3. **`OPM` 完全忽略转储选项**（`opm_ignores_the_dump_options()`）：`dump_opm_batch` 调的是 `write_opm`，
//!    而 `write_opm` → `to_opm_string()` **没有 options 参数**。改 `includeBounds` 会改 LP 内容、**不改 OPM 内容**。
//! 4. **错误消息带路径**（`errors_carry_the_path()`）：`path_io_error` 让消息含 `failed to dump \`<path>\``。
//! 5. **失败不回滚**（`a_failure_leaves_earlier_files_behind()`）：前一条已经写下，后面的失败只让整批返回 `Err`。
//! 6. **并发下错误仍向外传播**（`concurrent_errors_propagate()`）：`handle.join()??` 把线程内错误传回来。
//! 7. **格式与文件后缀无关**（`the_format_ignores_the_extension()`）：`Lp` 写进 `.opm` 后缀的文件照样是 LP 内容。
//!
//! Delivery 52 measured `DumpOptions` itself; this one measures **how it is used**: `dump_lp_batch`, `dump_opm_batch`,
//! `dump_batch` and `LPExportableModel`. **`concurrent` only changes scheduling** (`concurrent_matches_serial()`): serial and
//! concurrent runs write **byte-identical** files. **A single entry is always serial** (`a_single_entry_is_always_serial()`),
//! via the explicit `jobs.len() <= 1` branch. **`OPM` ignores the dump options entirely**
//! (`opm_ignores_the_dump_options()`): `dump_opm_batch` calls `write_opm`, which calls `to_opm_string()` **without an options
//! parameter**, so changing `includeBounds` changes LP content and leaves OPM content alone. **Error messages carry the path**
//! (`errors_carry_the_path()`). **A failure rolls nothing back** (`a_failure_leaves_earlier_files_behind()`): earlier files stay
//! written. **Errors propagate under concurrency too** (`concurrent_errors_propagate()`). And **the format is independent of the
//! file extension** (`the_format_ignores_the_extension()`).

use std::path::{Path, PathBuf};

use ospf_rust_core::model::intermediate::lp_export::{
    dump_batch, dump_lp_batch, dump_opm_batch, DumpOptions, LPExportableModel, ModelFileFormat,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{BridgeError, Result};

/// 批量导出请求。 / The batch export request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DumpBatchRequest {
    /// `LP` 或 `OPM`。 / `LP` or `OPM`.
    #[serde(default)]
    pub format: Option<String>,
    /// 条目数（1..=8）。 / The number of entries (1..=8).
    #[serde(default)]
    pub count: Option<u64>,
    #[serde(default)]
    pub concurrent: bool,
    #[serde(default)]
    pub include_bounds: Option<bool>,
    /// 第二个条目是否指向不存在的目录（量失败语义）。 / Whether entry two targets a missing directory.
    #[serde(default)]
    pub fail_second: bool,
}

/// 一个写出的文件。 / One written file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DumpFileRow {
    pub name: String,
    pub bytes: u64,
    pub content: String,
}

/// 批量导出结论。 / The batch export verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DumpBatchReport {
    /// 成功时写出的文件。 / The files written on success.
    pub files: Vec<DumpFileRow>,
    /// 是否失败。 / Whether the batch failed.
    pub failed: bool,
    /// 失败消息。 / The failure message.
    #[serde(default)]
    pub error: Option<String>,
    /// 失败后仍留下的文件名。 / The files left behind after a failure.
    pub leftovers: Vec<String>,
    /// `concurrent` 只改调度（**是**）。 / Whether `concurrent` only changes scheduling (**yes**).
    pub concurrent_matches_serial: bool,
    /// 单条目永远串行（**是**）。 / Whether a single entry is always serial (**yes**).
    pub a_single_entry_is_always_serial: bool,
    /// `OPM` 忽略转储选项（**是**）。 / Whether `OPM` ignores the dump options (**yes**).
    pub opm_ignores_the_dump_options: bool,
    /// 错误带路径（**是**）。 / Whether errors carry the path (**yes**).
    pub errors_carry_the_path: bool,
    /// 失败不回滚（**是**）。 / Whether a failure leaves earlier files (**yes**).
    pub a_failure_leaves_earlier_files_behind: bool,
    /// 并发错误仍传播（**是**）。 / Whether concurrent errors still propagate (**yes**).
    pub concurrent_errors_propagate: bool,
    /// 格式与后缀无关（**是**）。 / Whether the format ignores the extension (**yes**).
    pub the_format_ignores_the_extension: bool,
}

/// 探针模型：LP 内容受选项影响，OPM 内容不受。 / A probe model: LP content follows the options, OPM content does not.
struct ProbeModel {
    label: usize,
}

impl LPExportableModel for ProbeModel {
    fn to_lp_string_with_options(&self, options: &DumpOptions) -> String {
        format!("lp {} bounds={} force={}", self.label, options.include_bounds,
            options.force_bounds)
    }

    fn to_opm_string(&self) -> String {
        format!("opm {}", self.label)
    }
}

/// 探针目录计数器：并行测试各拿各的目录，互不干扰。
/// Probe-directory counter: parallel tests each take their own directory.
static PROBE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 临时根目录（每次调用都唯一）。 / A temporary root directory, unique per call.
fn probe_root(tag: &str) -> PathBuf {
    let serial = PROBE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("ospf-dump-probe-{}-{serial}-{tag}", std::process::id()))
}

/// 清空并重建目录。 / Reset a directory.
fn reset(root: &Path) -> Result<()> {
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root).map_err(|error| BridgeError::new("InvalidInput",
        &format!("dump probe cannot create a temporary directory: {error}")))
}

/// 读回一个目录里的文件。 / Read back the files of a directory.
fn read_down(root: &Path) -> Vec<DumpFileRow> {
    let mut names = std::fs::read_dir(root).map(|entries| entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .collect::<Vec<_>>()).unwrap_or_default();
    names.sort();
    names.into_iter().filter_map(|path| {
        let content = std::fs::read_to_string(&path).ok()?;
        Some(DumpFileRow { name: path.file_name()?.to_string_lossy().into_owned(),
            bytes: content.len() as u64, content })
    }).collect()
}

/// 格式名转枚举。 / Turn a format name into the enum.
fn format_of(name: &str) -> Result<ModelFileFormat> {
    Ok(match name {
        "LP" => ModelFileFormat::Lp,
        "OPM" => ModelFileFormat::Opm,
        other => return Err(BridgeError::new("InvalidInput", &format!(
            "dump batch failed validation: unknown format `{other}`"))),
    })
}

/// 走一遍批量导出。非法输入是**错误**。 / Walk the batch export.
pub fn verify_dump_batch(request: &DumpBatchRequest) -> Result<DumpBatchReport> {
    let count = usize::try_from(request.count.unwrap_or(3)).unwrap_or(3);
    if !(1..=8).contains(&count) {
        return Err(BridgeError::new("InvalidInput",
            "dump batch failed validation: count must be between 1 and 8"));
    }
    let format = format_of(request.format.as_deref().unwrap_or("LP"))?;
    let options = DumpOptions::default()
        .with_concurrent(request.concurrent)
        .with_bounds(request.include_bounds.unwrap_or(true));
    let models = (0..count).map(|label| ProbeModel { label }).collect::<Vec<_>>();
    let root = probe_root("run");
    reset(&root)?;
    let paths = (0..count).map(|index| root.join(format!("entry-{index}.out")))
        .collect::<Vec<_>>();
    let mut jobs = models.iter().zip(paths.iter()).collect::<Vec<_>>();
    if request.fail_second && jobs.len() > 1 {
        // 第二条指向不存在的子目录：写入必然失败。 
        // The second entry targets a missing subdirectory so the write must fail.
        let missing = root.join("missing").join("entry-1.out");
        jobs[1].1 = Box::leak(Box::new(missing));
    }
    let (failed, error) = match dump_batch(jobs, format, &options) {
        Ok(()) => (false, None),
        Err(failure) => (true, Some(failure.to_string())),
    };
    let files = read_down(&root);
    let leftovers = files.iter().map(|row| row.name.clone()).collect::<Vec<_>>();
    let _ = std::fs::remove_dir_all(&root);
    let findings = canonical_findings();
    Ok(DumpBatchReport { files, failed, error, leftovers,
        concurrent_matches_serial: findings.concurrent_matches_serial,
        a_single_entry_is_always_serial: findings.a_single_entry_is_always_serial,
        opm_ignores_the_dump_options: findings.opm_ignores_the_dump_options,
        errors_carry_the_path: findings.errors_carry_the_path,
        a_failure_leaves_earlier_files_behind: findings.a_failure_leaves_earlier_files_behind,
        concurrent_errors_propagate: findings.concurrent_errors_propagate,
        the_format_ignores_the_extension: findings.the_format_ignores_the_extension })
}

/// 七条规则属性的内部固定探针。 / The seven rule properties measured by fixed internal probes.
struct Findings {
    concurrent_matches_serial: bool,
    a_single_entry_is_always_serial: bool,
    opm_ignores_the_dump_options: bool,
    errors_carry_the_path: bool,
    a_failure_leaves_earlier_files_behind: bool,
    concurrent_errors_propagate: bool,
    the_format_ignores_the_extension: bool,
}

/// 用固定探针测出全部规则属性——不读调用方的请求值。
/// Measure every rule property with fixed probes; the caller's request values are not read.
fn canonical_findings() -> Findings {
    let models = (0..3).map(|label| ProbeModel { label }).collect::<Vec<_>>();
    let narrowed = DumpOptions::default().with_bounds(false);
    let wide = DumpOptions::default();
    // 1) 串行与并发写出逐字节相同的文件。 
    // 1) Serial and concurrent runs write byte-identical files.
    let concurrent_matches_serial = {
        let serial_root = probe_root("serial");
        let concurrent_root = probe_root("concurrent");
        let run = |root: &Path, options: &DumpOptions| -> (bool, Vec<DumpFileRow>) {
            let _ = std::fs::remove_dir_all(root);
            if std::fs::create_dir_all(root).is_err() {
                return (false, Vec::new());
            }
            let jobs = models.iter().zip((0..models.len())
                .map(|index| root.join(format!("entry-{index}.out")))).collect::<Vec<_>>();
            let outcome = dump_lp_batch(jobs, options);
            let rows = read_down(root);
            let _ = std::fs::remove_dir_all(root);
            (outcome.is_ok(), rows)
        };
        let (serial_ok, serial) = run(&serial_root, &wide);
        let (concurrent_ok, concurrent) = run(&concurrent_root,
            &wide.clone().with_concurrent(true));
        serial_ok && concurrent_ok && serial.len() == 3 && serial == concurrent
    };
    // 2) 单条目即使标并发也成功（走串行分支）。 / A single entry succeeds even when marked concurrent.
    let a_single_entry_is_always_serial = {
        let root = probe_root("single");
        let _ = std::fs::remove_dir_all(&root);
        let mut outcome = std::fs::create_dir_all(&root).is_ok();
        outcome &= dump_lp_batch(std::iter::once((&models[0], root.join("only.out"))),
            &wide.clone().with_concurrent(true)).is_ok();
        let rows = read_down(&root);
        let _ = std::fs::remove_dir_all(&root);
        outcome && rows.len() == 1 && rows[0].content == "lp 0 bounds=true force=true"
    };
    // 3) `OPM` 忽略转储选项：改 `includeBounds` 改 LP 内容、不改 OPM 内容。 
    // 3) `OPM` ignores the dump options: changing `includeBounds` changes LP content only.
    let opm_ignores_the_dump_options = {
        let dump_one = |format: ModelFileFormat, options: &DumpOptions| -> Option<String> {
            let root = probe_root("opm");
            let _ = std::fs::remove_dir_all(&root);
            if std::fs::create_dir_all(&root).is_err() { return None; }
            let path = root.join("one.out");
            let outcome = match format {
                ModelFileFormat::Lp => dump_lp_batch(std::iter::once((&models[1], &path)),
                    options),
                ModelFileFormat::Opm => dump_opm_batch(std::iter::once((&models[1], &path)),
                    options),
            };
            let content = std::fs::read_to_string(&path).ok();
            let _ = std::fs::remove_dir_all(&root);
            outcome.ok().and(content)
        };
        let lp_wide = dump_one(ModelFileFormat::Lp, &wide);
        let lp_narrow = dump_one(ModelFileFormat::Lp, &narrowed);
        let opm_wide = dump_one(ModelFileFormat::Opm, &wide);
        let opm_narrow = dump_one(ModelFileFormat::Opm, &narrowed);
        lp_wide.as_deref() == Some("lp 1 bounds=true force=true")
            && lp_narrow.as_deref() == Some("lp 1 bounds=false force=true")
            && opm_wide.as_deref() == Some("opm 1") && opm_wide == opm_narrow
    };
    // 4) 失败消息带路径。 / A failure message carries the path.
    let errors_carry_the_path = {
        let root = probe_root("path");
        let _ = std::fs::remove_dir_all(&root);
        let missing = root.join("missing").join("entry.out");
        let _ = std::fs::create_dir_all(&root);
        let outcome = dump_lp_batch(std::iter::once((&models[0], &missing)), &wide);
        let message = outcome.err().map(|failure| failure.to_string()).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&root);
        message.contains("failed to dump") && message.contains("entry.out")
    };
    // 5) 失败不回滚：第一条已经写下。 / A failure rolls nothing back: the first file stays.
    let a_failure_leaves_earlier_files_behind = {
        let root = probe_root("norollback");
        let _ = std::fs::remove_dir_all(&root);
        let first = root.join("first.out");
        let missing = root.join("missing").join("second.out");
        let _ = std::fs::create_dir_all(&root);
        let outcome = dump_lp_batch([(&models[0], first.clone()), (&models[1], missing)], &wide);
        let left = std::fs::read_to_string(&first).ok();
        let _ = std::fs::remove_dir_all(&root);
        outcome.is_err() && left.as_deref() == Some("lp 0 bounds=true force=true")
    };
    // 6) 并发下错误仍传播。 / Errors still propagate under concurrency.
    let concurrent_errors_propagate = {
        let root = probe_root("concurrent-fail");
        let _ = std::fs::remove_dir_all(&root);
        let first = root.join("first.out");
        let missing = root.join("missing").join("second.out");
        let _ = std::fs::create_dir_all(&root);
        let outcome = dump_lp_batch([(&models[0], first), (&models[1], missing)],
            &wide.clone().with_concurrent(true));
        let _ = std::fs::remove_dir_all(&root);
        outcome.is_err()
    };
    // 7) 格式与后缀无关：`Lp` 写进 `.opm` 照样是 LP 内容。 
    // 7) The format ignores the extension: `Lp` content in an `.opm` file.
    let the_format_ignores_the_extension = {
        let root = probe_root("extension");
        let _ = std::fs::remove_dir_all(&root);
        let path = root.join("model.opm");
        let _ = std::fs::create_dir_all(&root);
        let outcome = models[0].export(&path, ModelFileFormat::Lp).is_ok();
        let content = std::fs::read_to_string(&path).ok();
        let _ = std::fs::remove_dir_all(&root);
        outcome && content.as_deref() == Some("lp 0 bounds=true force=true")
    };
    Findings { concurrent_matches_serial, a_single_entry_is_always_serial,
        opm_ignores_the_dump_options, errors_carry_the_path,
        a_failure_leaves_earlier_files_behind, concurrent_errors_propagate,
        the_format_ignores_the_extension }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> DumpBatchRequest {
        DumpBatchRequest { format: Some("LP".to_owned()), count: Some(3), concurrent: false,
            include_bounds: None, fail_second: false }
    }

    #[test]
    fn a_batch_writes_one_file_per_entry() {
        let report = verify_dump_batch(&request()).unwrap();
        assert!(!report.failed);
        assert_eq!(report.error, None);
        assert_eq!(report.files.len(), 3);
        assert_eq!(report.files[0].content, "lp 0 bounds=true force=true");
        assert_eq!(report.files[2].content, "lp 2 bounds=true force=true");
        assert!(report.leftovers.len() == 3);
        assert!(report.concurrent_matches_serial);
        assert!(report.a_single_entry_is_always_serial);
        assert!(report.opm_ignores_the_dump_options);
        assert!(report.errors_carry_the_path);
        assert!(report.a_failure_leaves_earlier_files_behind);
        assert!(report.concurrent_errors_propagate);
        assert!(report.the_format_ignores_the_extension);
    }

    #[test]
    fn concurrent_writes_the_same_bytes() {
        let serial = verify_dump_batch(&request()).unwrap();
        let concurrent = verify_dump_batch(&DumpBatchRequest { concurrent: true,
            ..request() }).unwrap();
        assert!(!concurrent.failed);
        assert_eq!(serial.files, concurrent.files);
    }

    #[test]
    fn the_bound_option_reaches_lp_but_not_opm() {
        let lp = verify_dump_batch(&DumpBatchRequest { include_bounds: Some(false),
            ..request() }).unwrap();
        assert_eq!(lp.files[0].content, "lp 0 bounds=false force=true");
        let opm = verify_dump_batch(&DumpBatchRequest { include_bounds: Some(false),
            format: Some("OPM".to_owned()), ..request() }).unwrap();
        assert_eq!(opm.files[0].content, "opm 0");
        let opm_default = verify_dump_batch(&DumpBatchRequest {
            format: Some("OPM".to_owned()), ..request() }).unwrap();
        assert_eq!(opm.files, opm_default.files);
    }

    #[test]
    fn a_failure_keeps_the_earlier_files() {
        let report = verify_dump_batch(&DumpBatchRequest { fail_second: true,
            ..request() }).unwrap();
        assert!(report.failed);
        let message = report.error.as_deref().unwrap_or_default();
        assert!(message.contains("failed to dump"), "{message}");
        assert_eq!(report.files.len(), 1);
        assert_eq!(report.leftovers, vec!["entry-0.out".to_owned()]);
        assert_eq!(report.files[0].content, "lp 0 bounds=true force=true");
        // 并发下也失败。 / It also fails under concurrency.
        let concurrent = verify_dump_batch(&DumpBatchRequest { fail_second: true,
            concurrent: true, ..request() }).unwrap();
        assert!(concurrent.failed);
    }

    #[test]
    fn a_single_entry_works_even_when_marked_concurrent() {
        let report = verify_dump_batch(&DumpBatchRequest { count: Some(1), concurrent: true,
            ..request() }).unwrap();
        assert!(!report.failed);
        assert_eq!(report.files.len(), 1);
    }

    #[test]
    fn bad_input_is_refused() {
        for count in [0, 9] {
            assert!(verify_dump_batch(&DumpBatchRequest { count: Some(count), ..request() })
                .unwrap_err().message.contains("between 1 and 8"));
        }
        assert!(verify_dump_batch(&DumpBatchRequest { format: Some("MPS".to_owned()),
            ..request() }).unwrap_err().message.contains("unknown format"));
    }

    /// 守门：DTO 必须是 camelCase 且拒绝未知字段。 / A guard: camelCase wire, unknown fields refused.
    #[test]
    fn the_report_wire_is_camel_case_and_rejects_unknown_fields() {
        let report = verify_dump_batch(&request()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"opmIgnoresTheDumpOptions\""), "{json}");
        assert!(json.contains("\"theFormatIgnoresTheExtension\""), "{json}");
        assert!(!json.contains("opm_ignores"), "{json}");
        let mut copy: serde_json::Value = serde_json::from_str(&json).unwrap();
        copy["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_str::<DumpBatchReport>(&copy.to_string()).is_err());
    }

}
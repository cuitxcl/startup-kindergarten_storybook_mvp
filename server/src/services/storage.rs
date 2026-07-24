use std::{env, fs, path::PathBuf};

use serde::Serialize;
use uuid::Uuid;

const DEFAULT_STORAGE_ROOT: &str = "tmp";
const EXPORTS_CHILD_DIR: &str = "exports";
const GENERATED_IMAGES_CHILD_DIR: &str = "generated-images";
const STORAGE_ROOT_ENV: &str = "KINDLEAF_STORAGE_ROOT";
const EXPORTS_DIR_ENV: &str = "KINDLEAF_EXPORTS_DIR";
const GENERATED_IMAGES_DIR_ENV: &str = "KINDLEAF_GENERATED_IMAGES_DIR";
const EXPORT_MAX_BYTES_ENV: &str = "KINDLEAF_EXPORT_MAX_BYTES";
const GENERATED_IMAGE_MAX_BYTES_ENV: &str = "KINDLEAF_GENERATED_IMAGE_MAX_BYTES";
const WORKSPACE_STORAGE_QUOTA_BYTES_ENV: &str = "KINDLEAF_WORKSPACE_STORAGE_QUOTA_BYTES";
const USER_STORAGE_QUOTA_BYTES_ENV: &str = "KINDLEAF_USER_STORAGE_QUOTA_BYTES";
const PERSONAL_STORAGE_QUOTA_BYTES_ENV: &str = "KINDLEAF_PERSONAL_STORAGE_QUOTA_BYTES";
const SCHOOL_STORAGE_QUOTA_BYTES_ENV: &str = "KINDLEAF_SCHOOL_STORAGE_QUOTA_BYTES";
const STORAGE_QUOTA_WARNING_PERCENT_ENV: &str = "KINDLEAF_STORAGE_QUOTA_WARNING_PERCENT";
const DEFAULT_EXPORT_MAX_BYTES: usize = 50 * 1024 * 1024;
const DEFAULT_GENERATED_IMAGE_MAX_BYTES: usize = 15 * 1024 * 1024;
const DEFAULT_PERSONAL_STORAGE_QUOTA_BYTES: u64 = 200 * 1024 * 1024;
const DEFAULT_SCHOOL_STORAGE_QUOTA_BYTES: u64 = 5 * 1024 * 1024 * 1024;
const DEFAULT_STORAGE_QUOTA_WARNING_PERCENT: f64 = 80.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StorageSummary {
    pub backend: String,
    pub exports_dir: String,
    pub generated_images_dir: String,
    pub export_max_bytes: usize,
    pub generated_image_max_bytes: usize,
    pub filename_validation: bool,
    pub size_limit_enabled: bool,
    pub download_strategy: String,
    pub public_direct_access: bool,
    pub user_storage_quota_bytes: u64,
    pub personal_storage_quota_bytes: u64,
    pub school_storage_quota_bytes: u64,
    pub storage_quota_warning_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkspaceStorageQuotaSummary {
    pub workspace_id: Uuid,
    pub workspace_type: String,
    pub quota_bytes: u64,
    pub used_bytes: u64,
    pub remaining_bytes: u64,
    pub used_percent: f64,
    pub warning_percent: f64,
    pub warning: bool,
    pub exceeded: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UserStorageQuotaSummary {
    pub user_id: Uuid,
    pub quota_bytes: u64,
    pub used_bytes: u64,
    pub remaining_bytes: u64,
    pub used_percent: f64,
    pub warning_percent: f64,
    pub warning: bool,
    pub exceeded: bool,
    pub personal_workspace_count: u64,
}

pub fn storage_summary() -> StorageSummary {
    let export_max_bytes = export_max_bytes();
    let generated_image_max_bytes = generated_image_max_bytes();
    StorageSummary {
        backend: "local".to_string(),
        exports_dir: path_to_string(exports_dir()),
        generated_images_dir: path_to_string(generated_images_dir()),
        export_max_bytes,
        generated_image_max_bytes,
        filename_validation: true,
        size_limit_enabled: export_max_bytes > 0 || generated_image_max_bytes > 0,
        download_strategy: "authenticated_api".to_string(),
        public_direct_access: false,
        user_storage_quota_bytes: user_storage_quota_bytes(),
        personal_storage_quota_bytes: storage_quota_bytes_for_workspace_type("personal"),
        school_storage_quota_bytes: storage_quota_bytes_for_workspace_type("school"),
        storage_quota_warning_percent: storage_quota_warning_percent(),
    }
}

pub fn check_storage_writable() -> Result<(), String> {
    check_dir_writable(exports_dir(), "PDF 目录")?;
    check_dir_writable(generated_images_dir(), "插图目录")
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

pub fn save_export_file(file_name: &str, bytes: &[u8]) -> Result<String, String> {
    validate_export_file_name(file_name)?;
    validate_size(bytes, export_max_bytes(), "PDF")?;
    save_local_file(exports_dir(), file_name, bytes, "PDF").map(|_| format!("/exports/{file_name}"))
}

pub fn read_export_file(file_name: &str) -> Result<Vec<u8>, String> {
    validate_export_file_name(file_name)?;
    fs::read(local_export_path(file_name)).map_err(|err| format!("读取 PDF 失败：{err}"))
}

pub fn save_generated_image(file_name: &str, bytes: &[u8]) -> Result<String, String> {
    validate_generated_image_file_name(file_name)?;
    validate_size(bytes, generated_image_max_bytes(), "图片")?;
    save_local_file(generated_images_dir(), file_name, bytes, "图片")
        .map(|_| format!("/generated-images/{file_name}"))
}

pub fn file_size_for_url(file_url: &str) -> Result<u64, String> {
    let file_name = file_url
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "文件 URL 缺少文件名".to_string())?;
    let path = if file_url.starts_with("/exports/") {
        validate_export_file_name(file_name)?;
        local_export_path(file_name)
    } else if file_url.starts_with("/generated-images/") {
        validate_generated_image_file_name(file_name)?;
        local_generated_image_path_unchecked(file_name)
    } else {
        return Err("不支持的本地存储 URL".to_string());
    };
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .map_err(|err| format!("读取文件大小失败：{err}"))
}

pub fn delete_file_by_url(file_url: &str) -> Result<(), String> {
    let file_name = file_url
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "文件 URL 缺少文件名".to_string())?;
    let path = if file_url.starts_with("/exports/") {
        validate_export_file_name(file_name)?;
        local_export_path(file_name)
    } else if file_url.starts_with("/generated-images/") {
        validate_generated_image_file_name(file_name)?;
        local_generated_image_path_unchecked(file_name)
    } else {
        return Err("不支持的本地存储 URL".to_string());
    };
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("删除文件失败：{err}")),
    }
}

pub fn storage_quota_bytes_for_workspace_type(workspace_type: &str) -> u64 {
    if let Some(value) = configured_quota_bytes(env::var(WORKSPACE_STORAGE_QUOTA_BYTES_ENV).ok()) {
        return value;
    }
    match workspace_type.trim() {
        "school" => configured_quota_bytes(env::var(SCHOOL_STORAGE_QUOTA_BYTES_ENV).ok())
            .unwrap_or(DEFAULT_SCHOOL_STORAGE_QUOTA_BYTES),
        _ => configured_quota_bytes(env::var(PERSONAL_STORAGE_QUOTA_BYTES_ENV).ok())
            .unwrap_or(DEFAULT_PERSONAL_STORAGE_QUOTA_BYTES),
    }
}

pub fn user_storage_quota_bytes() -> u64 {
    configured_quota_bytes(env::var(USER_STORAGE_QUOTA_BYTES_ENV).ok())
        .unwrap_or_else(|| storage_quota_bytes_for_workspace_type("personal"))
}

pub fn storage_quota_warning_percent() -> f64 {
    env::var(STORAGE_QUOTA_WARNING_PERCENT_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|value| *value > 0.0 && *value <= 100.0)
        .unwrap_or(DEFAULT_STORAGE_QUOTA_WARNING_PERCENT)
}

pub fn workspace_storage_quota_summary(
    workspace_id: Uuid,
    workspace_type: &str,
    used_bytes: u64,
) -> WorkspaceStorageQuotaSummary {
    let quota_bytes = storage_quota_bytes_for_workspace_type(workspace_type);
    workspace_storage_quota_summary_with_limits(
        workspace_id,
        workspace_type,
        used_bytes,
        quota_bytes,
        storage_quota_warning_percent(),
    )
}

pub fn user_storage_quota_summary(
    user_id: Uuid,
    used_bytes: u64,
    personal_workspace_count: u64,
) -> UserStorageQuotaSummary {
    let quota_bytes = user_storage_quota_bytes();
    let warning_percent = storage_quota_warning_percent();
    let remaining_bytes = quota_bytes.saturating_sub(used_bytes);
    let used_percent = if quota_bytes == 0 {
        0.0
    } else {
        (used_bytes as f64 / quota_bytes as f64) * 100.0
    };
    UserStorageQuotaSummary {
        user_id,
        quota_bytes,
        used_bytes,
        remaining_bytes,
        used_percent,
        warning_percent,
        warning: used_percent >= warning_percent,
        exceeded: used_bytes >= quota_bytes,
        personal_workspace_count,
    }
}

pub fn read_generated_image(file_name: &str) -> Result<Vec<u8>, String> {
    validate_generated_image_file_name(file_name)?;
    fs::read(local_generated_image_path_unchecked(file_name))
        .map_err(|err| format!("读取图片失败：{err}"))
}

pub fn local_generated_image_path(file_name: &str) -> Result<PathBuf, String> {
    validate_generated_image_file_name(file_name)?;
    Ok(local_generated_image_path_unchecked(file_name))
}

fn local_generated_image_path_unchecked(file_name: &str) -> PathBuf {
    generated_images_dir().join(file_name)
}

fn local_export_path(file_name: &str) -> PathBuf {
    exports_dir().join(file_name)
}

fn exports_dir() -> PathBuf {
    configured_dir(EXPORTS_DIR_ENV, EXPORTS_CHILD_DIR)
}

fn generated_images_dir() -> PathBuf {
    configured_dir(GENERATED_IMAGES_DIR_ENV, GENERATED_IMAGES_CHILD_DIR)
}

fn configured_dir(override_env: &str, child_dir: &str) -> PathBuf {
    non_empty_env_path(override_env).unwrap_or_else(|| storage_root().join(child_dir))
}

fn storage_root() -> PathBuf {
    non_empty_env_path(STORAGE_ROOT_ENV).unwrap_or_else(|| PathBuf::from(DEFAULT_STORAGE_ROOT))
}

fn non_empty_env_path(key: &str) -> Option<PathBuf> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn export_max_bytes() -> usize {
    configured_max_bytes(
        env::var(EXPORT_MAX_BYTES_ENV).ok().as_deref(),
        DEFAULT_EXPORT_MAX_BYTES,
    )
}

fn generated_image_max_bytes() -> usize {
    configured_max_bytes(
        env::var(GENERATED_IMAGE_MAX_BYTES_ENV).ok().as_deref(),
        DEFAULT_GENERATED_IMAGE_MAX_BYTES,
    )
}

fn configured_max_bytes(value: Option<&str>, default_value: usize) -> usize {
    value
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default_value)
}

fn configured_quota_bytes(value: Option<String>) -> Option<u64> {
    value
        .as_deref()
        .map(str::trim)
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
}

fn workspace_storage_quota_summary_with_limits(
    workspace_id: Uuid,
    workspace_type: &str,
    used_bytes: u64,
    quota_bytes: u64,
    warning_percent: f64,
) -> WorkspaceStorageQuotaSummary {
    let remaining_bytes = quota_bytes.saturating_sub(used_bytes);
    let used_percent = if quota_bytes == 0 {
        0.0
    } else {
        (used_bytes as f64 / quota_bytes as f64) * 100.0
    };
    WorkspaceStorageQuotaSummary {
        workspace_id,
        workspace_type: workspace_type.to_string(),
        quota_bytes,
        used_bytes,
        remaining_bytes,
        used_percent,
        warning_percent,
        warning: used_percent >= warning_percent,
        exceeded: used_bytes >= quota_bytes,
    }
}

fn save_local_file(dir: PathBuf, file_name: &str, bytes: &[u8], label: &str) -> Result<(), String> {
    fs::create_dir_all(&dir).map_err(|err| format!("创建{label}目录失败：{err}"))?;
    fs::write(dir.join(file_name), bytes).map_err(|err| format!("写入{label}失败：{err}"))
}

fn check_dir_writable(dir: PathBuf, label: &str) -> Result<(), String> {
    fs::create_dir_all(&dir).map_err(|err| format!("创建{label}失败：{err}"))?;
    let probe = dir.join(format!(".kindleaf-readiness-{}.tmp", Uuid::new_v4()));
    fs::write(&probe, b"ok").map_err(|err| format!("写入{label}失败：{err}"))?;
    let bytes = fs::read(&probe).map_err(|err| format!("读取{label}探针失败：{err}"))?;
    if bytes != b"ok" {
        let _ = fs::remove_file(&probe);
        return Err(format!("{label}探针内容异常"));
    }
    fs::remove_file(&probe).map_err(|err| format!("清理{label}探针失败：{err}"))
}

fn validate_size(bytes: &[u8], max_bytes: usize, label: &str) -> Result<(), String> {
    if max_bytes == 0 || bytes.len() <= max_bytes {
        return Ok(());
    }
    Err(format!(
        "{label}文件过大：{} bytes，超过上限 {} bytes",
        bytes.len(),
        max_bytes
    ))
}

fn validate_export_file_name(file_name: &str) -> Result<(), String> {
    let Some(id) = file_name.strip_suffix(".pdf") else {
        return Err("PDF 文件名必须是 {uuid}.pdf".to_string());
    };
    Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| "PDF 文件名必须使用合法 UUID".to_string())
}

fn validate_generated_image_file_name(file_name: &str) -> Result<(), String> {
    let Some(name) = file_name.strip_suffix(".png") else {
        return Err("图片文件名必须是 {provider}-{uuid}.png".to_string());
    };
    let Some((provider, id)) = name.split_once('-') else {
        return Err("图片文件名必须包含 provider 前缀".to_string());
    };
    if !matches!(provider, "mock" | "seedream") {
        return Err("图片文件名 provider 只能是 mock 或 seedream".to_string());
    }
    Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| "图片文件名必须使用合法 UUID".to_string())
}

#[cfg(test)]
fn configured_dir_from_values(
    storage_root: Option<&str>,
    override_dir: Option<&str>,
    child_dir: &str,
) -> PathBuf {
    override_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(
                storage_root
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or(DEFAULT_STORAGE_ROOT),
            )
            .join(child_dir)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_urls_keep_existing_public_shape() {
        let export_id = Uuid::new_v4();
        let image_id = Uuid::new_v4();
        let export_file = format!("{export_id}.pdf");
        let image_file = format!("mock-{image_id}.png");
        let _ = fs::remove_file(local_export_path(&export_file));
        let _ = fs::remove_file(local_generated_image_path_unchecked(&image_file));
        assert_eq!(
            save_export_file(&export_file, b"%PDF").expect("export url should be created"),
            format!("/exports/{export_file}")
        );
        assert_eq!(
            save_generated_image(&image_file, b"png").expect("image url should be created"),
            format!("/generated-images/{image_file}")
        );
        let _ = fs::remove_file(local_export_path(&export_file));
        let _ = fs::remove_file(local_generated_image_path_unchecked(&image_file));
    }

    #[test]
    fn local_generated_image_path_uses_storage_dir() {
        let file_name = format!("mock-{}.png", Uuid::new_v4());
        assert!(
            local_generated_image_path(&file_name)
                .expect("image path should be valid")
                .ends_with(&file_name)
        );
    }

    #[test]
    fn storage_rejects_unsafe_export_file_names() {
        assert!(save_export_file("../secret.pdf", b"%PDF").is_err());
        assert!(save_export_file("storybook-1.pdf", b"%PDF").is_err());
        assert!(read_export_file("not-a-uuid.pdf").is_err());
    }

    #[test]
    fn storage_rejects_unsafe_generated_image_file_names() {
        assert!(save_generated_image("../mock-secret.png", b"png").is_err());
        assert!(
            save_generated_image("other-00000000-0000-0000-0000-000000000001.png", b"png").is_err()
        );
        assert!(read_generated_image("mock-page-1.png").is_err());
        assert!(
            local_generated_image_path("mock-00000000-0000-0000-0000-000000000001.txt").is_err()
        );
    }

    #[test]
    fn storage_rejects_files_over_configured_size_limit() {
        let bytes = b"abcd";
        assert!(validate_size(bytes, 4, "测试").is_ok());
        assert!(validate_size(bytes, 0, "测试").is_ok());
        let err = validate_size(bytes, 3, "测试").expect_err("oversized file should be rejected");
        assert!(err.contains("文件过大"));
        assert!(err.contains("4 bytes"));
        assert!(err.contains("3 bytes"));
    }

    #[test]
    fn storage_size_config_uses_default_for_invalid_values() {
        assert_eq!(configured_max_bytes(Some("123"), 10), 123);
        assert_eq!(configured_max_bytes(Some("  "), 10), 10);
        assert_eq!(configured_max_bytes(Some("abc"), 10), 10);
        assert_eq!(configured_max_bytes(None, 10), 10);
    }

    #[test]
    fn storage_summary_reports_current_boundaries() {
        let summary = storage_summary();
        assert!(summary.exports_dir.ends_with("exports"));
        assert!(summary.generated_images_dir.ends_with("generated-images"));
        assert_eq!(summary.backend, "local");
        assert_eq!(summary.download_strategy, "authenticated_api");
        assert!(!summary.public_direct_access);
        assert_eq!(summary.export_max_bytes, export_max_bytes());
        assert_eq!(
            summary.generated_image_max_bytes,
            generated_image_max_bytes()
        );
        assert!(summary.filename_validation);
        assert_eq!(
            summary.size_limit_enabled,
            summary.export_max_bytes > 0 || summary.generated_image_max_bytes > 0
        );
        assert!(summary.user_storage_quota_bytes > 0);
        assert!(summary.personal_storage_quota_bytes > 0);
        assert!(summary.school_storage_quota_bytes > summary.personal_storage_quota_bytes);
        assert!(summary.storage_quota_warning_percent > 0.0);
    }

    #[test]
    fn workspace_storage_quota_summary_reports_usage_state() {
        let workspace_id = Uuid::new_v4();
        let summary =
            workspace_storage_quota_summary_with_limits(workspace_id, "personal", 85, 100, 80.0);

        assert_eq!(summary.workspace_id, workspace_id);
        assert_eq!(summary.quota_bytes, 100);
        assert_eq!(summary.used_bytes, 85);
        assert_eq!(summary.remaining_bytes, 15);
        assert!(summary.warning);
        assert!(!summary.exceeded);
    }

    #[test]
    fn workspace_storage_quota_summary_marks_exceeded_at_limit() {
        let summary =
            workspace_storage_quota_summary_with_limits(Uuid::new_v4(), "school", 100, 100, 80.0);

        assert_eq!(summary.remaining_bytes, 0);
        assert!(summary.warning);
        assert!(summary.exceeded);
    }

    #[test]
    fn user_storage_quota_summary_reports_personal_usage_state() {
        let user_id = Uuid::new_v4();
        let summary = user_storage_quota_summary(user_id, 10, 1);

        assert_eq!(summary.user_id, user_id);
        assert_eq!(summary.used_bytes, 10);
        assert_eq!(summary.personal_workspace_count, 1);
        assert!(summary.quota_bytes > summary.used_bytes);
        assert!(!summary.exceeded);
    }

    #[test]
    fn storage_writability_probe_creates_and_cleans_probe_files() {
        check_storage_writable().expect("default storage dirs should be writable in tests");
        let leftovers = fs::read_dir(exports_dir())
            .expect("exports dir should exist")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".kindleaf-readiness-")
            })
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn storage_dir_config_prefers_specific_override() {
        assert_eq!(
            configured_dir_from_values(Some("var/kindleaf"), Some("custom/exports"), "exports"),
            PathBuf::from("custom/exports")
        );
    }

    #[test]
    fn storage_dir_config_falls_back_to_root_child() {
        assert_eq!(
            configured_dir_from_values(Some("var/kindleaf"), None, "generated-images"),
            PathBuf::from("var/kindleaf").join("generated-images")
        );
    }

    #[test]
    fn storage_dir_config_ignores_empty_values() {
        assert_eq!(
            configured_dir_from_values(Some(""), Some("  "), "exports"),
            PathBuf::from(DEFAULT_STORAGE_ROOT).join("exports")
        );
    }
}

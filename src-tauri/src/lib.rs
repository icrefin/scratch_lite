use anyhow::Result;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio::fs;

// AI execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiExecutionResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliStatus {
    pub supported: bool,
    pub installed: bool,
    pub path: Option<String>,
}

// Theme color customization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColors {
    pub bg: Option<String>,
    pub bg_secondary: Option<String>,
    pub bg_muted: Option<String>,
    pub bg_emphasis: Option<String>,
    pub text: Option<String>,
    pub text_muted: Option<String>,
    pub text_inverse: Option<String>,
    pub border: Option<String>,
    pub accent: Option<String>,
}

// Theme settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSettings {
    pub mode: String,
    pub custom_light_colors: Option<ThemeColors>,
    pub custom_dark_colors: Option<ThemeColors>,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            mode: "system".to_string(),
            custom_light_colors: None,
            custom_dark_colors: None,
        }
    }
}

// Editor font settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EditorFontSettings {
    pub base_font_family: Option<String>,
    pub base_font_size: Option<f32>,
    pub bold_weight: Option<i32>,
    pub line_height: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextDirection {
    Auto,
    Ltr,
    Rtl,
}

// App-wide settings (stored in app data directory)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub theme: ThemeSettings,
    #[serde(rename = "editorFont")]
    pub editor_font: Option<EditorFontSettings>,
    #[serde(rename = "textDirection")]
    pub text_direction: Option<TextDirection>,
    #[serde(rename = "editorWidth")]
    pub editor_width: Option<String>,
    #[serde(rename = "interfaceZoom")]
    pub interface_zoom: Option<f32>,
    #[serde(rename = "customEditorWidthPx")]
    pub custom_editor_width_px: Option<u32>,
    #[serde(rename = "ollamaModel")]
    pub ollama_model: Option<String>,
    #[serde(rename = "customColorsLight")]
    pub custom_colors_light: Option<std::collections::HashMap<String, String>>,
    #[serde(rename = "customColorsDark")]
    pub custom_colors_dark: Option<std::collections::HashMap<String, String>>,
    #[serde(rename = "recentFiles", default)]
    pub recent_files: Vec<String>,
}

// App state
pub struct AppState {
    pub settings: RwLock<Settings>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: RwLock::new(Settings::default()),
        }
    }
}

// Default user-configurable directories to ignore (common build/dependency folders)
const DEFAULT_IGNORED_DIRS: &[&str] = &[
    "node_modules",
    ".next",
    ".nuxt",
    "dist",
    "build",
    "out",
    "target",
    "vendor",
    "__pycache__",
    ".venv",
    "venv",
    ".cache",
    "coverage",
    ".svn",
    ".hg",
    "bower_components",
    ".turbo",
    ".parcel-cache",
];

// Settings file path (in app data directory)
fn get_settings_path(app: &AppHandle) -> Result<PathBuf> {
    let app_data = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data)?;
    Ok(app_data.join("settings.json"))
}

// Load settings from disk (without AppHandle, for use in menu builder)
fn load_settings_from_disk() -> Settings {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = std::path::PathBuf::from(home)
        .join("Library/Application Support/com.scratchlite.desktop/settings.json");

    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        Settings::default()
    }
}

// Load settings from disk
fn load_settings(app: &AppHandle) -> Settings {
    let path = match get_settings_path(app) {
        Ok(p) => p,
        Err(_) => return Settings::default(),
    };

    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    } else {
        Settings::default()
    }
}

// Save settings to disk
fn save_settings(app: &AppHandle, settings: &Settings) -> Result<()> {
    let path = get_settings_path(app)?;
    let content = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, content)?;
    Ok(())
}

// Utility: Check if a string is effectively empty
fn is_effectively_empty(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_whitespace() || c == '\u{00A0}' || c == '\u{FEFF}')
}

/// Strip YAML frontmatter (leading `---` ... `---` block) from content
fn strip_frontmatter(content: &str) -> &str {
    let trimmed = content.trim_start();
    if trimmed.starts_with("---") {
        if let Some(rest) = trimmed.strip_prefix("---") {
            if let Some(end) = rest.find("\n---") {
                let after_close = &rest[end + 4..];
                return after_close
                    .strip_prefix("\r\n")
                    .or_else(|| after_close.strip_prefix('\n'))
                    .unwrap_or(after_close);
            }
        }
    }
    content
}

// Extract title from markdown content
fn extract_title(content: &str) -> String {
    let body = strip_frontmatter(content);
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("# ") {
            let title = title.trim();
            if !is_effectively_empty(title) {
                return title.to_string();
            }
        }
        if !is_effectively_empty(trimmed) {
            return trimmed.chars().take(50).collect();
        }
    }
    "Untitled".to_string()
}

// TAURI COMMANDS

#[tauri::command]
fn get_parent_dir(file_path: String) -> Result<String, String> {
    let path = std::path::Path::new(&file_path);
    let parent = path
        .parent()
        .ok_or_else(|| "Failed to get parent directory".to_string())?;
    Ok(parent.to_string_lossy().to_string())
}

#[tauri::command]
async fn get_app_data_dir(app: AppHandle) -> Result<String, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().to_string())
}

#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    load_settings(&app)
}

#[tauri::command]
fn update_settings(app: AppHandle, new_settings: Settings) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut settings = state.settings.write().expect("settings write lock");
        *settings = new_settings.clone();
    }
    save_settings(&app, &new_settings).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_recent_files(app: AppHandle) -> Vec<String> {
    let state = app.state::<AppState>();
    let settings = state.settings.read().expect("settings read lock");
    settings.recent_files.clone()
}

#[tauri::command]
fn add_recent_file(app: AppHandle, path: String) -> Result<(), String> {
    let new_settings = {
        let state = app.state::<AppState>();
        let mut settings = state.settings.write().expect("settings write lock");
        // Remove if already exists
        settings.recent_files.retain(|p| p != &path);
        // Add to front
        settings.recent_files.insert(0, path);
        // Keep only last 10
        settings.recent_files.truncate(10);
        settings.clone()
    };
    save_settings(&app, &new_settings).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_default_ignored_patterns() -> Vec<String> {
    DEFAULT_IGNORED_DIRS.iter().map(|s| s.to_string()).collect()
}

#[tauri::command]
async fn write_file(path: String, contents: Vec<u8>) -> Result<(), String> {
    fs::write(&path, contents)
        .await
        .map_err(|_| "Failed to write file".to_string())
}

// Preview mode: file content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub title: String,
    pub modified: i64,
}

/// Validate a file path for direct file operations
fn validate_md_path(path: &str) -> Result<PathBuf, String> {
    let file_path = PathBuf::from(path);

    match file_path.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") => {}
        _ => return Err("Only .md and .markdown files are allowed".to_string()),
    }

    let canonical = file_path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve file path: {}", e))?;

    Ok(canonical)
}

#[tauri::command]
async fn read_file_direct(path: String) -> Result<FileContent, String> {
    let canonical = validate_md_path(&path)?;

    if !canonical.is_file() {
        return Err(format!("Not a file: {}", path));
    }

    let content = fs::read_to_string(&canonical)
        .await
        .map_err(|_| "Failed to read file".to_string())?;
    let metadata = fs::metadata(&canonical)
        .await
        .map_err(|_| "Failed to read metadata".to_string())?;

    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let title = extract_title(&content);

    Ok(FileContent {
        path,
        content,
        title,
        modified,
    })
}

#[tauri::command]
async fn save_file_direct(path: String, content: String) -> Result<FileContent, String> {
    let canonical = validate_md_path(&path)?;

    if !canonical.is_file() {
        return Err(format!("Not a file: {}", path));
    }

    fs::write(&canonical, &content)
        .await
        .map_err(|_| "Failed to write file".to_string())?;

    let metadata = fs::metadata(&canonical)
        .await
        .map_err(|_| "Failed to read metadata".to_string())?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let title = extract_title(&content);

    Ok(FileContent {
        path,
        content,
        title,
        modified,
    })
}

#[tauri::command]
fn copy_to_clipboard(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_clipboard_image(
    base64_data: String,
    target_dir: String,
) -> Result<String, String> {
    if base64_data.trim().is_empty() {
        return Err("Clipboard data is empty".to_string());
    }

    let image_data = base64::engine::general_purpose::STANDARD
        .decode(&base64_data)
        .map_err(|_| "Failed to decode base64 image data".to_string())?;

    if image_data.is_empty() {
        return Err("Decoded image data is empty".to_string());
    }

    let target_dir = PathBuf::from(&target_dir);
    let assets_dir = target_dir.join("assets");
    fs::create_dir_all(&assets_dir)
        .await
        .map_err(|e| e.to_string())?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut target_name = format!("screenshot-{}.png", timestamp);
    let mut counter = 1;
    let mut target_path = assets_dir.join(&target_name);

    while target_path.exists() {
        target_name = format!("screenshot-{}-{}.png", timestamp, counter);
        target_path = assets_dir.join(&target_name);
        counter += 1;
    }

    fs::write(&target_path, &image_data)
        .await
        .map_err(|_| "Failed to write image".to_string())?;

    Ok(format!("assets/{}", target_name))
}

#[tauri::command]
async fn copy_image_to_assets(
    source_path: String,
    target_dir: String,
) -> Result<String, String> {
    let source = PathBuf::from(&source_path);
    if !source.exists() {
        return Err("Source image file does not exist".to_string());
    }

    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .ok_or("Invalid file extension")?;

    const ALLOWED_IMAGE_EXTENSIONS: &[&str] = &[
        "jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "tiff", "tif", "ico", "avif",
    ];
    let ext_lower = extension.to_lowercase();
    if !ALLOWED_IMAGE_EXTENSIONS.contains(&ext_lower.as_str()) {
        return Err("Only image files can be copied to assets".to_string());
    }

    let original_name = source
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("image");

    // Sanitize the filename
    let sanitized_name: String = original_name
        .chars()
        .filter(|c| *c != '\u{00A0}' && *c != '\u{FEFF}')
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            _ => c,
        })
        .collect();

    let target_dir = PathBuf::from(&target_dir);
    let assets_dir = target_dir.join("assets");
    fs::create_dir_all(&assets_dir)
        .await
        .map_err(|e| e.to_string())?;

    let mut target_name = format!("{}.{}", sanitized_name, extension);
    let mut counter = 1;
    let mut target_path = assets_dir.join(&target_name);

    while target_path.exists() {
        target_name = format!("{}-{}.{}", sanitized_name, counter, extension);
        target_path = assets_dir.join(&target_name);
        counter += 1;
    }

    fs::copy(&source, &target_path)
        .await
        .map_err(|_| "Failed to copy image".to_string())?;

    Ok(format!("assets/{}", target_name))
}

#[tauri::command]
async fn open_folder_dialog(
    app: AppHandle,
    default_path: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut builder = app.dialog().file().add_filter("Markdown", &["md", "markdown"]);

        if let Some(path) = default_path {
            builder = builder.set_directory(path);
        }

        builder.blocking_pick_file()
    })
    .await
    .map_err(|e| format!("Dialog task failed: {}", e))?;

    Ok(result.map(|p| p.to_string()))
}

#[tauri::command]
async fn open_file_dialog(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let result = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Markdown", &["md", "markdown"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| format!("Dialog task failed: {}", e))?;

    Ok(result.map(|p| p.to_string()))
}

#[tauri::command]
async fn open_in_file_manager(path: String) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);
    if !path_buf.exists() {
        return Err("Path does not exist".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        let windows_path = path.replace("/", "\\");
        std::process::Command::new("explorer")
            .arg(&format!("/select,{}", windows_path))
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = path_buf.parent() {
            std::process::Command::new("xdg-open")
                .arg(parent.to_string_lossy().as_ref())
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        return Err("Unsupported platform".to_string());
    }

    Ok(())
}

#[tauri::command]
async fn open_url_safe(url: String) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|e| format!("Invalid URL: {}", e))?;

    match parsed.scheme() {
        "http" | "https" | "mailto" => {}
        scheme => {
            return Err(format!(
                "URL scheme '{}' is not allowed. Only http, https, and mailto are permitted.",
                scheme
            ))
        }
    }

    open::that(&url).map_err(|e| format!("Failed to open URL: {}", e))
}

#[tauri::command]
async fn open_file_preview(app: AppHandle, path: String) -> Result<(), String> {
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    // Emit event to main window to open the file
    let _ = app.emit("open-file", &path);
    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.show();
        let _ = main_window.set_focus();
    }
    Ok(())
}

// CLI commands

fn get_expanded_path() -> String {
    let system_path = std::env::var("PATH").unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_else(|_| String::new());

    if home.is_empty() {
        return system_path;
    }

    let candidate_dirs = vec![
        format!("{home}/.nvm/versions/node"),
        format!("{home}/.fnm/node-versions"),
        format!("{home}/.local/share/mise/installs/node"),
    ];
    let static_dirs = vec![
        format!("{home}/.bun/bin"),
        format!("{home}/.volta/bin"),
        format!("{home}/.local/bin"),
        "/usr/local/bin".to_string(),
        "/opt/homebrew/bin".to_string(),
    ];

    let mut expanded = Vec::new();

    for dir in static_dirs {
        expanded.push(dir);
    }

    for base in &candidate_dirs {
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let bin_path = entry.path().join("bin");
                if bin_path.exists() {
                    expanded.push(bin_path.to_string_lossy().to_string());
                }
            }
        }
    }

    expanded.push(system_path);
    expanded.join(":")
}

fn no_window_cmd(program: &str) -> std::process::Command {
    let cmd = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = cmd;
        cmd.creation_flags(0x08000000);
        cmd
    }
    #[cfg(not(target_os = "windows"))]
    {
        cmd
    }
}

fn check_cli_exists(command_name: &str, path: &str) -> Result<bool, String> {
    let which_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };

    let check_output = no_window_cmd(which_cmd)
        .arg(command_name)
        .env("PATH", path)
        .output()
        .map_err(|e| format!("Failed to check for {} CLI: {}", command_name, e))?;

    Ok(check_output.status.success())
}

#[cfg(target_os = "macos")]
const SCRATCH_CLI_MARKER: &str = "# SCRATCH_CLI_WRAPPER";

#[cfg(target_os = "macos")]
fn cli_target_path() -> PathBuf {
    if let Ok(path_var) = std::env::var("PATH") {
        if path_var.split(':').any(|p| p == "/opt/homebrew/bin") {
            return PathBuf::from("/opt/homebrew/bin/scratch");
        }
    }
    if std::env::consts::ARCH == "aarch64" {
        return PathBuf::from("/opt/homebrew/bin/scratch");
    }
    PathBuf::from("/usr/local/bin/scratch")
}

#[tauri::command]
fn get_cli_status() -> Result<CliStatus, String> {
    #[cfg(not(target_os = "macos"))]
    return Ok(CliStatus { supported: false, installed: false, path: None });

    #[cfg(target_os = "macos")]
    {
        let target = cli_target_path();
        if !target.exists() && target.symlink_metadata().is_err() {
            return Ok(CliStatus { supported: true, installed: false, path: None });
        }
        let content = std::fs::read_to_string(&target).unwrap_or_default();
        if !content.contains(SCRATCH_CLI_MARKER) {
            return Ok(CliStatus { supported: true, installed: false, path: None });
        }
        let current_exe = std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !current_exe.is_empty() && !content.contains(&current_exe) {
            return Ok(CliStatus { supported: true, installed: false, path: None });
        }
        Ok(CliStatus {
            supported: true,
            installed: true,
            path: Some(target.to_string_lossy().into_owned()),
        })
    }
}

#[tauri::command]
fn install_cli() -> Result<String, String> {
    #[cfg(not(target_os = "macos"))]
    return Err("CLI install is only supported on macOS".to_string());

    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::PermissionsExt;

        let target = cli_target_path();

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }

        if target.exists() || target.symlink_metadata().is_ok() {
            let content = std::fs::read_to_string(&target).unwrap_or_default();
            if !content.contains(SCRATCH_CLI_MARKER) {
                return Err(format!(
                    "A different 'scratch' command already exists at {}. Remove it manually to install the Scratch CLI.",
                    target.display()
                ));
            }
            std::fs::remove_file(&target)
                .map_err(|e| format!("Failed to remove existing file: {}", e))?;
        }

        let exe_path = std::env::current_exe()
            .map_err(|e| format!("Cannot find exe path: {}", e))?;

        let exe_str = exe_path.to_string_lossy();
        let escaped_exe = format!("'{}'", exe_str.replace('\'', "'\\''"));

        let script = format!(
            "#!/bin/sh\n{}\nnohup {} \"$@\" >/dev/null 2>&1 &\n",
            SCRATCH_CLI_MARKER,
            escaped_exe
        );
        std::fs::write(&target, script.as_bytes())
            .map_err(|e| format!("Failed to write CLI script: {}", e))?;

        let mut perms = std::fs::metadata(&target)
            .map_err(|e| format!("Failed to read permissions: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&target, perms)
            .map_err(|e| format!("Failed to set permissions: {}", e))?;

        Ok(target.to_string_lossy().into_owned())
    }
}

#[tauri::command]
fn uninstall_cli() -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    return Ok(());

    #[cfg(target_os = "macos")]
    {
        let target = cli_target_path();
        if target.exists() || target.symlink_metadata().is_ok() {
            let content = std::fs::read_to_string(&target).unwrap_or_default();
            if !content.contains(SCRATCH_CLI_MARKER) {
                return Err(format!(
                    "File at {} was not installed by Scratch. Refusing to remove.",
                    target.display()
                ));
            }
            std::fs::remove_file(&target)
                .map_err(|e| format!("Failed to remove CLI script: {}", e))?;
        }
        Ok(())
    }
}

// AI CLI check commands

#[tauri::command]
async fn ai_check_claude_cli() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let path = get_expanded_path();
        check_cli_exists("claude", &path)
    })
    .await
    .map_err(|e| format!("Failed to check Claude CLI: {}", e))?
}

#[tauri::command]
async fn ai_check_codex_cli() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let path = get_expanded_path();
        check_cli_exists("codex", &path)
    })
    .await
    .map_err(|e| format!("Failed to check Codex CLI: {}", e))?
}

#[tauri::command]
async fn ai_check_opencode_cli() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let path = get_expanded_path();
        check_cli_exists("opencode", &path)
    })
    .await
    .map_err(|e| format!("Failed to check OpenCode CLI: {}", e))?
}

#[tauri::command]
async fn ai_check_ollama_cli() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let path = get_expanded_path();
        check_cli_exists("ollama", &path)
    })
    .await
    .map_err(|e| format!("Failed to check Ollama CLI: {}", e))?
}

// Shared AI CLI execution
async fn execute_ai_cli(
    cli_name: &str,
    command: String,
    args: Vec<String>,
    stdin_input: String,
    not_found_msg: String,
    current_dir: Option<String>,
    extra_env: Option<Vec<(String, String)>>,
) -> Result<AiExecutionResult, String> {
    use std::io::Write;
    use std::process::{Child, Stdio};

    let cli_name = cli_name.to_string();
    let timeout_duration = std::time::Duration::from_secs(300);
    let shared_child: Arc<std::sync::Mutex<Option<Child>>> = Arc::new(std::sync::Mutex::new(None));
    let child_for_task = Arc::clone(&shared_child);
    let cli_name_task = cli_name.clone();

    let mut task = tauri::async_runtime::spawn_blocking(move || {
        let path = get_expanded_path();
        match check_cli_exists(&command, &path) {
            Ok(false) => {
                return AiExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some(not_found_msg),
                };
            }
            Err(e) => {
                return AiExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                };
            }
            Ok(true) => {}
        }

        let mut cmd = no_window_cmd(&command);
        cmd.env("PATH", &path);
        if let Some(dir) = &current_dir {
            cmd.current_dir(dir);
        }
        if let Some(env_pairs) = &extra_env {
            for (key, value) in env_pairs {
                cmd.env(key, value);
            }
        }
        for arg in &args {
            cmd.arg(arg);
        }
        let process = match cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(p) => p,
            Err(e) => {
                return AiExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to execute {}: {}", cli_name_task, e)),
                };
            }
        };

        if let Ok(mut guard) = child_for_task.lock() {
            *guard = Some(process);
        } else {
            return AiExecutionResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to lock {} process handle", cli_name_task)),
            };
        }

        let stdin_handle = child_for_task
            .lock()
            .ok()
            .and_then(|mut g| g.as_mut().and_then(|p| p.stdin.take()));

        if let Some(mut stdin) = stdin_handle {
            if let Err(e) = stdin.write_all(stdin_input.as_bytes()) {
                if let Ok(mut g) = child_for_task.lock() {
                    if let Some(ref mut p) = *g {
                        let _ = p.kill();
                        let _ = p.wait();
                    }
                }
                return AiExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to write to {} stdin: {}", cli_name_task, e)),
                };
            }
        } else {
            if let Ok(mut g) = child_for_task.lock() {
                if let Some(ref mut p) = *g {
                    let _ = p.kill();
                    let _ = p.wait();
                }
            }
            return AiExecutionResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to open stdin for {}", cli_name_task)),
            };
        }

        let stdout_handle = child_for_task
            .lock()
            .ok()
            .and_then(|mut g| g.as_mut().and_then(|p| p.stdout.take()));
        let stderr_handle = child_for_task
            .lock()
            .ok()
            .and_then(|mut g| g.as_mut().and_then(|p| p.stderr.take()));

        use std::io::Read;

        let mut stdout_str = String::new();
        if let Some(mut out) = stdout_handle {
            let _ = out.read_to_string(&mut stdout_str);
        }

        let mut stderr_str = String::new();
        if let Some(mut err) = stderr_handle {
            let _ = err.read_to_string(&mut stderr_str);
        }

        let success = child_for_task
            .lock()
            .ok()
            .and_then(|mut g| g.as_mut().and_then(|p| p.wait().ok()))
            .map(|s| s.success())
            .unwrap_or(false);

        let ansi_re = regex::Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\].*?\x07").unwrap();
        let stdout_clean = ansi_re.replace_all(&stdout_str, "").to_string();
        let stderr_clean = ansi_re.replace_all(&stderr_str, "").trim().to_string();

        if success {
            AiExecutionResult {
                success: true,
                output: stdout_clean,
                error: None,
            }
        } else {
            AiExecutionResult {
                success: false,
                output: stdout_clean,
                error: Some(stderr_clean),
            }
        }
    });

    let result = match tokio::time::timeout(timeout_duration, &mut task).await {
        Ok(join_result) => {
            join_result.map_err(|e| format!("Failed to join {} blocking task: {}", cli_name, e))?
        }
        Err(_) => {
            if let Ok(mut guard) = shared_child.lock() {
                if let Some(ref mut process) = *guard {
                    let _ = process.kill();
                }
            }

            match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                Ok(join_result) => {
                    if let Err(e) = join_result {
                        return Err(format!(
                            "Failed to join {} blocking task after timeout: {}",
                            cli_name, e
                        ));
                    }
                }
                Err(_) => {
                    return Err(format!(
                        "{} CLI timed out and failed to exit after kill signal",
                        cli_name
                    ));
                }
            }

            AiExecutionResult {
                success: false,
                output: String::new(),
                error: Some(format!("{} CLI timed out after 5 minutes", cli_name)),
            }
        }
    };

    Ok(result)
}

// Validate a path is a markdown file
fn validate_md_path_for_ai(path: &str) -> Result<PathBuf, String> {
    let file_path = PathBuf::from(path);
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !ext.eq_ignore_ascii_case("md") && !ext.eq_ignore_ascii_case("markdown") {
        return Err("AI editing is only supported for markdown files".to_string());
    }
    let canonical = file_path
        .canonicalize()
        .map_err(|_| "Invalid file path".to_string())?;
    Ok(canonical)
}

#[tauri::command]
async fn ai_execute_claude(
    file_path: String,
    prompt: String,
) -> Result<AiExecutionResult, String> {
    let canonical = validate_md_path_for_ai(&file_path)?;

    execute_ai_cli(
        "Claude",
        "claude".to_string(),
        vec![
            canonical.to_string_lossy().to_string(),
            "--dangerously-skip-permissions".to_string(),
            "--print".to_string(),
        ],
        prompt,
        "Claude CLI not found. Please install it from https://claude.ai/code".to_string(),
        None,
        None,
    )
    .await
}

#[tauri::command]
async fn ai_execute_codex(file_path: String, prompt: String) -> Result<AiExecutionResult, String> {
    let stdin_input = format!(
        "Edit only this markdown file: {file_path}\n\
         Apply the user's instructions below directly to that file.\n\
         Do not create, delete, rename, or modify any other files.\n\
         User instructions:\n\
         {prompt}"
    );

    execute_ai_cli(
        "Codex",
        "codex".to_string(),
        vec![
            "exec".to_string(),
            "--skip-git-repo-check".to_string(),
            "--dangerously-bypass-approvals-and-sandbox".to_string(),
            "-".to_string(),
        ],
        stdin_input,
        "Codex CLI not found. Please install it from https://github.com/openai/codex".to_string(),
        None,
        None,
    )
    .await
}

#[tauri::command]
async fn ai_execute_opencode(
    file_path: String,
    prompt: String,
) -> Result<AiExecutionResult, String> {
    let canonical = validate_md_path_for_ai(&file_path)?;
    let parent_dir = canonical
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    let run_prompt = format!(
        "Edit the attached markdown file in place.\n\
         Do not create, delete, rename, or modify any other files.\n\
         User instructions:\n\
         {}",
        prompt
    );

    execute_ai_cli(
        "OpenCode",
        "opencode".to_string(),
        vec![
            "run".to_string(),
            "--file".to_string(),
            canonical.to_string_lossy().to_string(),
            "--".to_string(),
            run_prompt,
        ],
        String::new(),
        "OpenCode CLI not found. Please install it from https://opencode.ai".to_string(),
        Some(parent_dir),
        Some(vec![
            (
                "OPENCODE_PERMISSION".to_string(),
                r#"{"*":"allow","bash":"deny","task":"deny","webfetch":"deny","websearch":"deny","codesearch":"deny","skill":"deny","external_directory":"deny","doom_loop":"deny"}"#.to_string(),
            ),
        ]),
    )
    .await
}

#[tauri::command]
async fn ai_execute_ollama(
    file_path: String,
    prompt: String,
    model: String,
) -> Result<AiExecutionResult, String> {
    let canonical = validate_md_path_for_ai(&file_path)?;

    let file_content = tokio::fs::read_to_string(&canonical)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let stdin_input = format!(
        "You are a markdown editor. Edit the markdown content below according to the user's instructions.\n\
         Return ONLY the complete edited markdown content.\n\
         Do NOT include any explanation, commentary, or code fences around the output.\n\
         Do NOT add ```markdown or ``` wrappers.\n\n\
         Current markdown content:\n{file_content}\n\n\
         User instructions:\n{prompt}"
    );

    let trimmed = model.trim();
    let model_name = if trimmed.is_empty() {
        "qwen3:8b".to_string()
    } else {
        trimmed.to_string()
    };

    if !model_name.contains("cloud") {
        let mn = model_name.clone();
        let available = tauri::async_runtime::spawn_blocking(move || {
            let path = get_expanded_path();
            let mut cmd = no_window_cmd("ollama");
            cmd.env("PATH", &path);
            cmd.args(["show", &mn]);
            cmd.stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            match cmd.status() {
                Ok(status) => status.success(),
                Err(_) => false,
            }
        })
        .await
        .unwrap_or(false);

        if !available {
            return Ok(AiExecutionResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Model '{}' is not installed. Run: ollama pull {}",
                    model_name, model_name
                )),
            });
        }
    }

    let result = execute_ai_cli(
        "Ollama",
        "ollama".to_string(),
        vec!["run".to_string(), model_name.clone()],
        stdin_input,
        "Ollama CLI not found. Please install it from https://ollama.com".to_string(),
        None,
        None,
    )
    .await?;

    if !result.success {
        if let Some(ref err) = result.error {
            let err_lower = err.to_lowercase();
            if err_lower.contains("file does not exist")
                || err_lower.contains("pull model manifest")
                || err_lower.contains("model not found")
                || err_lower.contains("model does not exist")
            {
                return Ok(AiExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!(
                        "Model '{}' not found. Run `ollama pull {}` in your terminal to download it.",
                        model_name, model_name
                    )),
                });
            }
            if err.contains("401") || err.contains("Unauthorized") {
                return Ok(AiExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some("Authentication required. Run `ollama login` in your terminal to sign in.".to_string()),
                });
            }
        }
    }

    if result.success {
        let edited_content = result.output.trim().to_string();
        if edited_content.is_empty() {
            return Ok(AiExecutionResult {
                success: false,
                output: String::new(),
                error: Some("Ollama returned empty output. Please try again.".to_string()),
            });
        }
        tokio::fs::write(&canonical, edited_content.as_bytes())
            .await
            .map_err(|e| format!("Failed to write edited file: {}", e))?;

        Ok(AiExecutionResult {
            success: true,
            output: "Note edited successfully with Ollama.".to_string(),
            error: None,
        })
    } else {
        Ok(result)
    }
}

/// Check if a file extension is a supported markdown extension
fn is_markdown_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| {
            let lower = s.to_ascii_lowercase();
            lower == "md" || lower == "markdown"
        })
        .unwrap_or(false)
}

// Find the first .md file in CLI arguments
fn find_first_markdown_file(args: &[String], cwd: &str) -> Option<String> {
    for arg in args.iter().skip(1) {
        if arg.starts_with('-') {
            continue;
        }

        let path = if PathBuf::from(arg).is_absolute() {
            PathBuf::from(arg)
        } else {
            PathBuf::from(cwd).join(arg)
        };

        if is_markdown_extension(&path) && path.is_file() {
            return Some(path.to_string_lossy().to_string());
        }
    }
    None
}

// Handle CLI arguments: open .md files in main window
fn handle_cli_args(app: &AppHandle, args: &[String], cwd: &str) {
    if let Some(path) = find_first_markdown_file(args, cwd) {
        let _ = app.emit("open-file", &path);
        if let Some(main_window) = app.get_webview_window("main") {
            let _ = main_window.show();
            let _ = main_window.set_focus();
        }
    } else {
        if let Some(main_window) = app.get_webview_window("main") {
            let _ = main_window.show();
            let _ = main_window.set_focus();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            handle_cli_args(app, &args, &cwd);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .menu(|handle| {
            // App menu (macOS: first submenu becomes the app-name menu)
            let settings = MenuItemBuilder::with_id("settings", "Settings...")
                .accelerator("CmdOrCtrl+,")
                .build(handle)?;

            let app_menu = SubmenuBuilder::new(handle, "ScratchLite")
                .item(&PredefinedMenuItem::about(handle, Some("About ScratchLite"), None)?)
                .separator()
                .item(&settings)
                .separator()
                .item(&PredefinedMenuItem::services(handle, None)?)
                .separator()
                .item(&PredefinedMenuItem::hide(handle, None)?)
                .item(&PredefinedMenuItem::hide_others(handle, None)?)
                .item(&PredefinedMenuItem::show_all(handle, None)?)
                .separator()
                .item(&PredefinedMenuItem::quit(handle, None)?)
                .build()?;

            // File menu
            let new_file = MenuItemBuilder::with_id("new_file", "New File")
                .accelerator("CmdOrCtrl+N")
                .build(handle)?;
            let open = MenuItemBuilder::with_id("open", "Open...")
                .accelerator("CmdOrCtrl+O")
                .build(handle)?;
            let save = MenuItemBuilder::with_id("save", "Save")
                .accelerator("CmdOrCtrl+S")
                .build(handle)?;
            let save_as = MenuItemBuilder::with_id("save_as", "Save As...")
                .accelerator("CmdOrCtrl+Shift+S")
                .build(handle)?;
            let print = MenuItemBuilder::with_id("print", "Print...")
                .accelerator("CmdOrCtrl+Shift+P")
                .build(handle)?;

            // Open Recent submenu
            let recent_files = {
                let settings = load_settings_from_disk();
                settings.recent_files
            };

            let mut recent_menu = SubmenuBuilder::new(handle, "Open Recent");
            if recent_files.is_empty() {
                let no_recent = MenuItemBuilder::with_id("recent_empty", "No Recent Files")
                    .enabled(false)
                    .build(handle)?;
                recent_menu = recent_menu.item(&no_recent);
            } else {
                for (i, file_path) in recent_files.iter().enumerate() {
                    let file_name = std::path::Path::new(file_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(file_path);
                    let item = MenuItemBuilder::with_id(format!("recent_{}", i), file_name)
                        .build(handle)?;
                    recent_menu = recent_menu.item(&item);
                }
            }
            let recent_menu = recent_menu.build()?;

            let file_menu = SubmenuBuilder::new(handle, "File")
                .item(&new_file)
                .item(&open)
                .separator()
                .item(&recent_menu)
                .separator()
                .item(&save)
                .item(&save_as)
                .separator()
                .item(&print)
                .separator()
                .item(&PredefinedMenuItem::close_window(handle, None)?)
                .build()?;

            // Edit menu
            let edit_menu = SubmenuBuilder::new(handle, "Edit")
                .item(&PredefinedMenuItem::undo(handle, None)?)
                .item(&PredefinedMenuItem::redo(handle, None)?)
                .separator()
                .item(&PredefinedMenuItem::cut(handle, None)?)
                .item(&PredefinedMenuItem::copy(handle, None)?)
                .item(&PredefinedMenuItem::paste(handle, None)?)
                .separator()
                .item(&PredefinedMenuItem::select_all(handle, None)?)
                .build()?;

            // View menu
            let zoom_in = MenuItemBuilder::with_id("zoom_in", "Zoom In")
                .accelerator("CmdOrCtrl+=")
                .build(handle)?;
            let zoom_out = MenuItemBuilder::with_id("zoom_out", "Zoom Out")
                .accelerator("CmdOrCtrl+-")
                .build(handle)?;
            let zoom_reset = MenuItemBuilder::with_id("zoom_reset", "Actual Size")
                .accelerator("CmdOrCtrl+0")
                .build(handle)?;
            let toggle_focus = MenuItemBuilder::with_id("toggle_focus", "Toggle Focus Mode")
                .accelerator("CmdOrCtrl+Shift+Enter")
                .build(handle)?;
            let toggle_source = MenuItemBuilder::with_id("toggle_source", "Toggle Source Mode")
                .accelerator("CmdOrCtrl+Shift+M")
                .build(handle)?;

            let view_menu = SubmenuBuilder::new(handle, "View")
                .item(&zoom_in)
                .item(&zoom_out)
                .item(&zoom_reset)
                .separator()
                .item(&toggle_focus)
                .item(&toggle_source)
                .separator()
                .item(&PredefinedMenuItem::fullscreen(handle, None)?)
                .build()?;

            // Window menu
            let window_menu = SubmenuBuilder::new(handle, "Window")
                .item(&PredefinedMenuItem::minimize(handle, None)?)
                .build()?;

            MenuBuilder::new(handle)
                .item(&app_menu)
                .item(&file_menu)
                .item(&edit_menu)
                .item(&view_menu)
                .item(&window_menu)
                .build()
        })
        .on_menu_event(|app, event| {
            let id = event.id().as_ref().to_string();
            // Handle recent file clicks
            if id.starts_with("recent_") {
                if let Ok(index) = id.strip_prefix("recent_").unwrap_or("0").parse::<usize>() {
                    let state = app.state::<AppState>();
                    let settings = state.settings.read().expect("settings read lock");
                    if let Some(file_path) = settings.recent_files.get(index) {
                        let _ = app.emit("open-file", file_path);
                    }
                }
            } else {
                let _ = app.emit("menu-event", id);
            }
        })
        .setup(|app| {
            let settings = load_settings(app.handle());

            // Check CLI args for a file to open
            let args: Vec<String> = std::env::args().collect();
            let pending = if args.len() > 1 {
                let cwd = std::env::current_dir()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                find_first_markdown_file(&args, &cwd)
            } else {
                None
            };

            app.manage(AppState {
                settings: RwLock::new(settings),
            });

            // Show the window
            if let Some(main_window) = app.get_webview_window("main") {
                let _ = main_window.show();
            }

            // Emit pending file path after a delay (page may not be loaded yet)
            if let Some(file) = pending {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let _ = app_handle.emit("open-file", &file);
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                let app = window.app_handle();
                for path in paths {
                    if is_markdown_extension(path) && path.is_file() {
                        let _ = app.emit("open-file", path.to_string_lossy().as_ref());
                        if let Some(main_window) = app.get_webview_window("main") {
                            let _ = main_window.set_focus();
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_parent_dir,
            get_app_data_dir,
            get_settings,
            update_settings,
            get_recent_files,
            add_recent_file,
            get_default_ignored_patterns,
            write_file,
            read_file_direct,
            save_file_direct,
            copy_to_clipboard,
            save_clipboard_image,
            copy_image_to_assets,
            open_folder_dialog,
            open_file_dialog,
            open_in_file_manager,
            open_url_safe,
            open_file_preview,
            ai_check_claude_cli,
            ai_check_codex_cli,
            ai_check_opencode_cli,
            ai_check_ollama_cli,
            ai_execute_claude,
            ai_execute_codex,
            ai_execute_opencode,
            ai_execute_ollama,
            install_cli,
            uninstall_cli,
            get_cli_status,
            set_title_bar_theme,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, _event| {
        #[cfg(target_os = "macos")]
        {
            if let tauri::RunEvent::Opened { urls } = _event {
                for url in urls {
                    if let Ok(path) = url.to_file_path() {
                        if is_markdown_extension(&path) && path.is_file() {
                            let path_str = path.to_string_lossy().to_string();
                            // If app is already running, emit immediately
                            let _ = _app_handle.emit("open-file", &path_str);
                            // If this is a fresh launch, the frontend may not be ready yet,
                            // so emit again after a delay to ensure the listener is registered
                            let app_clone = _app_handle.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(2000));
                                let _ = app_clone.emit("open-file", &path_str);
                            });
                            if let Some(main_window) = _app_handle.get_webview_window("main") {
                                let _ = main_window.set_focus();
                            }
                        }
                    }
                }
            }
        }
    });
}

#[cfg(target_os = "windows")]
mod windows_title_bar {
    use tauri::WebviewWindow;

    #[allow(non_snake_case)]
    mod dwm {
        pub const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
        pub const DWMWA_CAPTION_COLOR: u32 = 35;
        pub const DWMWA_BORDER_COLOR: u32 = 34;

        extern "system" {
            pub fn DwmSetWindowAttribute(
                hwnd: isize,
                attr: u32,
                value: *const std::ffi::c_void,
                size: u32,
            ) -> i32;
        }
    }

    pub fn apply_title_bar_theme(window: &WebviewWindow, is_dark: bool, rgb: (u8, u8, u8)) {
        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        let hwnd = hwnd.0 as isize;

        let (r, g, b) = rgb;
        let caption_color: u32 =
            ((b as u32) << 16) | ((g as u32) << 8) | (r as u32);

        unsafe {
            let set_attr = |attr: u32, value: *const std::ffi::c_void, size: u32| {
                let _ = dwm::DwmSetWindowAttribute(hwnd, attr, value, size);
            };

            let dark_mode: i32 = if is_dark { 1 } else { 0 };
            set_attr(
                dwm::DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_mode as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<i32>() as u32,
            );
            set_attr(
                dwm::DWMWA_CAPTION_COLOR,
                &caption_color as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
            set_attr(
                dwm::DWMWA_BORDER_COLOR,
                &caption_color as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }
}

#[tauri::command]
fn set_title_bar_theme(
    app: AppHandle,
    is_dark: bool,
    r: u8,
    g: u8,
    b: u8,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        for (label, window) in app.webview_windows() {
            if label == "main" || label.starts_with("preview-") {
                windows_title_bar::apply_title_bar_theme(&window, is_dark, (r, g, b));
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, is_dark, r, g, b);
    }
    Ok(())
}

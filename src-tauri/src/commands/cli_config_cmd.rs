// CLI 工具配置命令
//
// - check_*_installed：探测 CLI 是否已安装
// - write_claude_code_config：写 ~/.claude/settings.json 的 env 字段
// - write_codex_cli_config：写 ~/.codex/{config.toml, auth.json}

use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri::command;
use toml_edit::{value, DocumentMut};

/// 检查 CLI 是否已安装（用 `<cmd> --version` 探测）
fn cli_installed(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|r| r.status.success())
        .unwrap_or(false)
}

// 安全限制
const MAX_CONFIG_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
const MAX_URL_LENGTH: usize = 2048;
const MAX_API_KEY_LENGTH: usize = 1024;

/// 读取 JSON 文件，文件不存在时返回 `{}`
fn read_or_empty_json(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }

    // 安全检查：文件大小限制
    let metadata = fs::metadata(path)
        .map_err(|e| format!("读取 {} 元数据失败: {e}", path.display()))?;
    if metadata.len() > MAX_CONFIG_FILE_SIZE {
        return Err(format!("配置文件过大: {} bytes", metadata.len()));
    }

    let content = fs::read_to_string(path)
        .map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
    serde_json::from_str(&content).map_err(|e| format!("解析 {} 失败: {e}", path.display()))
}

/// 序列化 JSON 并写入文件
fn write_pretty_json(path: &Path, value: &Value) -> Result<(), String> {
    let content =
        serde_json::to_string_pretty(value).map_err(|e| format!("序列化 JSON 失败: {e}"))?;
    fs::write(path, content).map_err(|e| format!("写入 {} 失败: {e}", path.display()))
}

#[command]
pub async fn check_claude_code_installed() -> Result<bool, String> {
    Ok(cli_installed("claude"))
}

#[command]
pub async fn check_codex_cli_installed() -> Result<bool, String> {
    Ok(cli_installed("codex"))
}

#[command]
pub async fn write_claude_code_config(base_url: String, api_key: String) -> Result<String, String> {
    // 安全检查：参数长度限制
    if base_url.len() > MAX_URL_LENGTH {
        return Err(format!("base_url 过长: {} 字符", base_url.len()));
    }
    if api_key.len() > MAX_API_KEY_LENGTH {
        return Err(format!("api_key 过长: {} 字符", api_key.len()));
    }

    // 安全检查：URL 格式验证
    if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
        return Err("base_url 必须以 http:// 或 https:// 开头".to_string());
    }

    let home_dir = dirs::home_dir().ok_or("无法获取用户目录")?;
    let config_path = home_dir.join(".claude").join("settings.json");

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }

    let mut config = read_or_empty_json(&config_path)?;

    // 确保根是 object
    if !config.is_object() {
        config = serde_json::json!({});
    }

    // 写入 env.ANTHROPIC_BASE_URL / env.ANTHROPIC_AUTH_TOKEN（保留其他已有 env 字段）
    let env = config
        .as_object_mut()
        .unwrap()
        .entry("env")
        .or_insert_with(|| serde_json::json!({}));
    if !env.is_object() {
        *env = serde_json::json!({});
    }
    let env_obj = env.as_object_mut().unwrap();
    env_obj.insert("ANTHROPIC_BASE_URL".into(), Value::String(base_url));
    env_obj.insert("ANTHROPIC_AUTH_TOKEN".into(), Value::String(api_key));

    write_pretty_json(&config_path, &config)?;
    Ok(format!("已写入配置到 {}", config_path.display()))
}

#[command]
pub async fn write_codex_cli_config(
    base_url: String,
    api_key: String,
    model: Option<String>,
) -> Result<String, String> {
    // 安全检查：参数长度限制
    if base_url.len() > MAX_URL_LENGTH {
        return Err(format!("base_url 过长: {} 字符", base_url.len()));
    }
    if api_key.len() > MAX_API_KEY_LENGTH {
        return Err(format!("api_key 过长: {} 字符", api_key.len()));
    }
    if let Some(ref m) = model {
        if m.len() > 256 {
            return Err(format!("model 过长: {} 字符", m.len()));
        }
    }

    // 安全检查：URL 格式验证
    if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
        return Err("base_url 必须以 http:// 或 https:// 开头".to_string());
    }

    let home_dir = dirs::home_dir().ok_or("无法获取用户目录")?;
    let codex_dir = home_dir.join(".codex");
    fs::create_dir_all(&codex_dir).map_err(|e| format!("创建目录失败: {e}"))?;

    let config_path = codex_dir.join("config.toml");
    let auth_path = codex_dir.join("auth.json");

    // 1. config.toml：写入 model_provider="custom" + [model_providers.custom] 配置
    let mut doc: DocumentMut = if config_path.exists() {
        fs::read_to_string(&config_path)
            .map_err(|e| format!("读取 config.toml 失败: {e}"))?
            .parse()
            .map_err(|e| format!("解析 config.toml 失败: {e}"))?
    } else {
        DocumentMut::new()
    };

    doc["model_provider"] = value("custom");
    if let Some(model_name) = model {
        doc["model"] = value(model_name);
    }

    // 确保 [model_providers.custom] 表存在
    if !doc.contains_key("model_providers") {
        doc["model_providers"] = toml_edit::table();
    }
    if doc["model_providers"]
        .as_table()
        .and_then(|t| t.get("custom"))
        .is_none()
    {
        doc["model_providers"]["custom"] = toml_edit::table();
    }

    let custom = doc["model_providers"]["custom"]
        .as_table_mut()
        .ok_or("无法获取 model_providers.custom 表")?;
    custom.insert("name", value("custom"));
    custom.insert("base_url", value(base_url));
    custom.insert("wire_api", value("responses"));
    custom.insert("requires_openai_auth", value(true));

    fs::write(&config_path, doc.to_string())
        .map_err(|e| format!("写入 config.toml 失败: {e}"))?;

    // 2. auth.json：写 OPENAI_API_KEY（保留其他字段）
    let mut auth = read_or_empty_json(&auth_path)?;
    auth["OPENAI_API_KEY"] = Value::String(api_key);
    write_pretty_json(&auth_path, &auth)?;

    Ok(format!(
        "已写入配置到:\n- {}\n- {}",
        config_path.display(),
        auth_path.display()
    ))
}

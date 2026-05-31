// Kiro IDE 相关功能

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

// ===== 辅助函数 =====

/// 根据 startUrl 计算 clientIdHash（与 Kiro IDE 源码一致）
fn calculate_client_id_hash(start_url: &str) -> String {
    let input = format!(r#"{{"startUrl":"{start_url}"}}"#);
    let mut hasher = Sha1::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// 检查文件是否为符号链接（安全性检查，参考 Kiro IDE）
fn assert_not_symlink(path: &std::path::Path) -> Result<(), String> {
    if path.exists() {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|e| format!("Failed to read metadata: {}", e))?;
        if metadata.file_type().is_symlink() {
            return Err("Token file is a symbolic link".to_string());
        }
    }
    Ok(())
}

/// 设置文件权限为 0600（仅所有者可读写，仅 Unix 系统）
#[cfg(unix)]
fn set_file_permissions(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)
        .map_err(|e| format!("Failed to read file metadata: {}", e))?
        .permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(path, perms)
        .map_err(|e| format!("Failed to set file permissions: {}", e))?;
    Ok(())
}

/// Windows 系统不需要设置权限
#[cfg(not(unix))]
fn set_file_permissions(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}

// ===== Kiro IDE 本地 Token =====

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroLocalToken {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<String>,
    pub auth_method: Option<String>,
    pub provider: Option<String>,
    // Social 专用
    pub profile_arn: Option<String>,
    // IdC 专用
    pub client_id_hash: Option<String>,
    pub region: Option<String>,
    // 注意：Kiro IDE 不在 kiro-auth-token.json 中存储 startUrl
    // startUrl 包含在 clientSecret JWT 的 initiateLoginUri 字段中
}

/// `IdC` 客户端注册信息 (从 {clientIdHash}.json 读取)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientRegistration {
    pub client_id: String,
    pub client_secret: String,
    pub expires_at: Option<String>,
}

#[tauri::command]
pub async fn get_kiro_local_token() -> Option<KiroLocalToken> {
    tokio::task::spawn_blocking(|| {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .ok()?;
        let path = std::path::Path::new(&home)
            .join(".aws")
            .join("sso")
            .join("cache")
            .join("kiro-auth-token.json");

        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    })
    .await
    .ok()
    .flatten()
}

/// 读取 `IdC` 客户端注册信息
pub async fn get_client_registration(client_id_hash: &str) -> Option<ClientRegistration> {
    // 安全检查：防止路径遍历攻击
    // 只允许字母、数字、下划线和连字符
    if !client_id_hash.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        log::warn!("[安全] 检测到非法的 client_id_hash: {}", client_id_hash);
        return None;
    }

    let hash = client_id_hash.to_string();
    tokio::task::spawn_blocking(move || {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .ok()?;
        let path = std::path::Path::new(&home)
            .join(".aws")
            .join("sso")
            .join("cache")
            .join(format!("{hash}.json"));

        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    })
    .await
    .ok()
    .flatten()
}

// ===== 从 Kiro IDE 导入账号 =====

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroAccountInfo {
    pub email: String,
    pub provider: String,
    pub auth_method: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<String>,
    // Social 专用
    pub profile_arn: Option<String>,
    // IdC 专用
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub client_id_hash: Option<String>,
    pub region: Option<String>,
    // 注意：不需要 start_url 字段
    // startUrl 包含在 clientSecret JWT 的 initiateLoginUri 字段中
    // AWS SSO OIDC API 会自动从 JWT 中解析
}

/// 读取 Kiro IDE 中的所有账号
#[tauri::command]
pub async fn read_kiro_accounts() -> Result<Vec<KiroAccountInfo>, String> {
    tokio::task::spawn_blocking(|| {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map_err(|_| "无法获取用户目录")?;

        let cache_dir = std::path::Path::new(&home)
            .join(".aws")
            .join("sso")
            .join("cache");

        if !cache_dir.exists() {
            return Err("未找到 Kiro IDE 缓存目录".to_string());
        }

        let mut accounts = Vec::new();

        // 读取主 token 文件
        let token_path = cache_dir.join("kiro-auth-token.json");
        if token_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&token_path) {
                if let Ok(token) = serde_json::from_str::<KiroLocalToken>(&content) {
                    let auth_method = token.auth_method.as_deref().unwrap_or("social");
                    let provider = token
                        .provider
                        .clone()
                        .unwrap_or_else(|| "Google".to_string());

                    let mut account = KiroAccountInfo {
                        email: String::new(), // 需要通过 API 获取
                        provider: provider.clone(),
                        auth_method: auth_method.to_string(),
                        access_token: token.access_token.clone(),
                        refresh_token: token.refresh_token.clone(),
                        expires_at: token.expires_at.clone(),
                        profile_arn: token.profile_arn.clone(),
                        client_id: None,
                        client_secret: None,
                        client_id_hash: token.client_id_hash.clone(),
                        region: token.region.clone(),
                    };

                    // 如果是 IdC 账号，读取 client registration
                    if auth_method == "IdC" {
                        if let Some(ref hash) = token.client_id_hash {
                            let client_path = cache_dir.join(format!("{hash}.json"));
                            if let Ok(client_content) = std::fs::read_to_string(&client_path) {
                                if let Ok(client_reg) =
                                    serde_json::from_str::<ClientRegistration>(&client_content)
                                {
                                    account.client_id = Some(client_reg.client_id);
                                    account.client_secret = Some(client_reg.client_secret);
                                }
                            }
                        }
                    }

                    accounts.push(account);
                }
            }
        }

        if accounts.is_empty() {
            return Err("未找到 Kiro IDE 账号，请先在 Kiro IDE 中登录".to_string());
        }

        Ok(accounts)
    })
    .await
    .map_err(|e| format!("读取失败: {e}"))?
}

// ===== 切换账号 =====

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchAccountResult {
    pub success: bool,
    pub message: String,
}

/// 切换账号参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchAccountParams {
    pub access_token: String,
    pub refresh_token: String,
    pub provider: String,
    #[serde(default)]
    pub auth_method: Option<String>,
    // Social 专用
    #[serde(default)]
    pub profile_arn: Option<String>,
    // IdC 专用
    #[serde(default)]
    pub start_url: Option<String>, // Enterprise 必须提供，BuilderId 不需要
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
}

/// 切换 Kiro 账号（原子写入 Token 文件，无需重启 IDE）
#[tauri::command]
pub async fn switch_kiro_account(
    params: SwitchAccountParams,
) -> Result<SwitchAccountResult, String> {
    tokio::task::spawn_blocking(move || {
        let auth_method = params.auth_method.unwrap_or_else(|| "social".to_string());
        let access_token = params.access_token;
        let refresh_token = params.refresh_token;
        let provider = params.provider;
        let profile_arn = params.profile_arn;
        let start_url = params.start_url;
        let client_id = params.client_id;
        let client_secret = params.client_secret;
        let region = params.region;

        // 获取 token 目录
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map_err(|_| "Cannot find home directory")?;

        let dir_path = std::path::Path::new(&home)
            .join(".aws")
            .join("sso")
            .join("cache");

        std::fs::create_dir_all(&dir_path)
            .map_err(|e| format!("Failed to create directory: {e}"))?;

        let file_path = dir_path.join("kiro-auth-token.json");
        let expires_at = chrono::Utc::now() + chrono::Duration::hours(1);

        // 根据 auth_method 构建 token 数据
        let token_data = if auth_method == "IdC" {
            // 确定 startUrl
            let actual_start_url = if provider == "BuilderId" {
                "https://view.awsapps.com/start".to_string()
            } else if provider == "Enterprise" {
                start_url
                    .clone()
                    .ok_or("Enterprise 账号必须提供 start_url")?
            } else {
                return Err(format!("未知的 IdC Provider: {provider}"));
            };

            // 计算 clientIdHash
            let hash = calculate_client_id_hash(&actual_start_url);

            serde_json::json!({
                "accessToken": access_token,
                "refreshToken": refresh_token,
                "expiresAt": expires_at.to_rfc3339(),
                "authMethod": "IdC",
                "provider": provider,
                "clientIdHash": hash,
                "region": region.ok_or("IdC 账号必须提供 region")?,
            })
        } else {
            let arn = profile_arn
                .unwrap_or_else(|| crate::commands::common::KIRO_SOCIAL_PROFILE_ARN.to_string());
            serde_json::json!({
                "accessToken": access_token,
                "refreshToken": refresh_token,
                "profileArn": arn,
                "expiresAt": expires_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                "authMethod": "social",
                "provider": provider
            })
        };

        let content = serde_json::to_string_pretty(&token_data)
            .map_err(|e| format!("Failed to serialize: {e}"))?;

        // 安全检查：确保目标文件不是符号链接
        assert_not_symlink(&file_path)?;

        // 原子写入：先写临时文件，再 rename
        let temp_file_path = dir_path.join("kiro-auth-token.json.tmp");

        // 安全检查：如果临时文件已存在，确保不是符号链接后删除
        if temp_file_path.exists() {
            assert_not_symlink(&temp_file_path)?;
            std::fs::remove_file(&temp_file_path)
                .map_err(|e| format!("Failed to remove existing temp file: {e}"))?;
        }

        // 使用 OpenOptions 安全写入（create_new 确保不跟随符号链接）
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_file_path)
            .map_err(|e| format!("Failed to create temp file: {e}"))?;
        file.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write temp file: {e}"))?;
        drop(file);

        std::fs::rename(&temp_file_path, &file_path)
            .map_err(|e| format!("Failed to rename file: {e}"))?;

        // 设置文件权限为 0600（仅 Unix 系统）
        set_file_permissions(&file_path).ok();

        // IdC 账号还需要写入 Client Registration 文件
        if auth_method == "IdC" {
            if let (Some(cid), Some(csec)) = (client_id, client_secret) {
                // 确定 startUrl
                let actual_start_url = if provider == "BuilderId" {
                    "https://view.awsapps.com/start".to_string()
                } else if provider == "Enterprise" {
                    start_url.ok_or("Enterprise 账号必须提供 start_url")?
                } else {
                    return Err(format!("未知的 IdC Provider: {provider}"));
                };

                // 计算 clientIdHash
                let hash = calculate_client_id_hash(&actual_start_url);

                let client_reg_path = dir_path.join(format!("{hash}.json"));
                let client_reg_temp_path = dir_path.join(format!("{hash}.json.tmp"));
                let client_expires = chrono::Utc::now() + chrono::Duration::days(90);
                let client_reg_data = serde_json::json!({
                    "clientId": cid,
                    "clientSecret": csec,
                    "expiresAt": client_expires.to_rfc3339()
                });
                let client_reg_content = serde_json::to_string_pretty(&client_reg_data)
                    .map_err(|e| format!("Failed to serialize client registration: {e}"))?;

                // 安全检查：确保目标文件不是符号链接
                assert_not_symlink(&client_reg_path)?;

                // 安全检查：如果临时文件已存在，确保不是符号链接后删除
                if client_reg_temp_path.exists() {
                    assert_not_symlink(&client_reg_temp_path)?;
                    std::fs::remove_file(&client_reg_temp_path)
                        .map_err(|e| format!("Failed to remove existing client temp file: {e}"))?;
                }

                // 使用 OpenOptions 安全写入（create_new 确保不跟随符号链接）
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&client_reg_temp_path)
                    .map_err(|e| format!("Failed to create client temp file: {e}"))?;
                file.write_all(client_reg_content.as_bytes())
                    .map_err(|e| format!("Failed to write client temp file: {e}"))?;
                drop(file);

                std::fs::rename(&client_reg_temp_path, &client_reg_path)
                    .map_err(|e| format!("Failed to rename client registration: {e}"))?;

                // 设置文件权限为 0600（仅 Unix 系统）
                set_file_permissions(&client_reg_path).ok();
            } else {
                return Err("IdC 账号必须提供 client_id 和 client_secret".to_string());
            }
        }

        Ok(SwitchAccountResult {
            success: true,
            message: format!("Switched to {provider} ({auth_method}) account"),
        })
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

/// IDE 安装检测结果
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdeInstallationInfo {
    pub ide_installed: bool,
    pub ide_path: Option<String>,
    pub ide_executable_exists: bool,
    pub config_dir_exists: bool,
    pub error_message: Option<String>,
}

/// 检测 Kiro IDE 是否安装
#[tauri::command]
pub async fn check_ide_installation() -> IdeInstallationInfo {
    tokio::task::spawn_blocking(|| {
        let (ide_path, ide_exists) = detect_kiro_ide_executable();
        
        // 检查配置目录是否存在
        let config_exists = check_kiro_config_dir();

        let ide_installed = ide_exists && config_exists;
        
        // 生成详细的错误提示
        let error_message = if !ide_installed {
            if !ide_exists && !config_exists {
                Some("未检测到默认路径的 Kiro IDE 可执行文件和配置文件。\n\n请先安装并登录 Kiro IDE，或在「设置」→「通用」中配置「自定义 Kiro IDE 安装路径」。".to_string())
            } else if !ide_exists {
                Some("未检测到默认路径的 Kiro IDE 可执行文件。\n\n请检查 IDE 是否已安装，或在「设置」→「通用」中配置「自定义 Kiro IDE 安装路径」。".to_string())
            } else if !config_exists {
                Some("Kiro IDE 已安装，但尚未首次登录。\n\n请先在 Kiro IDE 中完成首次登录后再使用切换功能。".to_string())
            } else {
                None
            }
        } else {
            None
        };

        IdeInstallationInfo {
            ide_installed,
            ide_path,
            ide_executable_exists: ide_exists,
            config_dir_exists: config_exists,
            error_message,
        }
    })
    .await
    .unwrap_or(IdeInstallationInfo {
        ide_installed: false,
        ide_path: None,
        ide_executable_exists: false,
        config_dir_exists: false,
        error_message: Some("检测 IDE 安装状态时发生错误".to_string()),
    })
}

/// 检查 Kiro IDE 配置文件是否存在且包含有效 token
fn check_kiro_config_dir() -> bool {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"));

    if let Ok(home_dir) = home {
        let cache_dir = std::path::Path::new(&home_dir)
            .join(".aws")
            .join("sso")
            .join("cache");

        let token_file = cache_dir.join("kiro-auth-token.json");

        // 1. 检查文件是否存在
        if !token_file.exists() {
            return false;
        }

        // 2. 读取并解析文件内容
        if let Ok(content) = std::fs::read_to_string(&token_file) {
            if let Ok(token_data) = serde_json::from_str::<KiroLocalToken>(&content) {
                // 3. 验证必须有 access_token 和 refresh_token
                if token_data.access_token.is_none() || token_data.refresh_token.is_none() {
                    return false;
                }

                // 4. 验证必须有 auth_method
                let auth_method = match token_data.auth_method.as_deref() {
                    Some(method) => method,
                    None => return false,
                };

                // 5. 验证必须有 provider
                if token_data.provider.is_none() {
                    return false;
                }

                // 6. 根据 auth_method 验证特定字段
                match auth_method {
                    "social" => {
                        // Social 账号需要 profileArn
                        token_data.profile_arn.is_some()
                    }
                    "IdC" => {
                        // IdC 账号 (BuilderId/Enterprise) 需要 clientIdHash 和 region
                        token_data.client_id_hash.is_some() && token_data.region.is_some()
                    }
                    _ => false, // 未知的 auth_method
                }
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    }
}

/// 检测 IDE 可执行文件
fn detect_kiro_ide_executable() -> (Option<String>, bool) {
    let candidates = get_kiro_ide_paths();
    for path in candidates {
        if path.exists() {
            return (Some(path.to_string_lossy().to_string()), true);
        }
    }
    (None, false)
}

/// 检测配置文件是否存在（用于切换账号前验证）
#[tauri::command]
pub async fn check_kiro_config_files(auth_method: String, client_id_hash: Option<String>) -> Result<bool, String> {
    tokio::task::spawn_blocking(move || {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map_err(|_| "无法获取用户目录".to_string())?;

        let cache_dir = std::path::Path::new(&home)
            .join(".aws")
            .join("sso")
            .join("cache");

        // 检查主 token 文件
        let token_file = cache_dir.join("kiro-auth-token.json");
        if !token_file.exists() {
            return Ok(false);
        }

        // 如果是 IdC 账号，还需检查 client registration 文件
        if auth_method == "idc" {
            if let Some(hash) = client_id_hash {
                let client_file = cache_dir.join(format!("{}.json", hash));
                if !client_file.exists() {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

/// 获取 Kiro IDE 候选路径
pub fn get_kiro_ide_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();

    // 1. 优先检查自定义路径（如果用户在设置中配置了）
    if let Ok(settings) = crate::commands::app_settings_cmd::get_app_settings_inner() {
        if let Some(custom_path) = settings.custom_kiro_path {
            let path_buf = std::path::PathBuf::from(&custom_path);
            paths.push(path_buf);
        }
    }

    // 2. 如果没有自定义路径，检查默认路径
    if cfg!(target_os = "windows") {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            paths.push(
                std::path::PathBuf::from(local_app_data)
                    .join("Programs")
                    .join("Kiro")
                    .join("Kiro.exe"),
            );
        }
    } else if cfg!(target_os = "macos") {
        // macOS: Kiro.app 安装在 /Applications
        paths.push(std::path::PathBuf::from("/Applications/Kiro.app"));
    } else {
        // Linux: 可能在多个位置
        paths.push(std::path::PathBuf::from("/usr/bin/kiro"));

        if let Ok(home) = std::env::var("HOME") {
            paths.push(
                std::path::PathBuf::from(&home)
                    .join(".local")
                    .join("bin")
                    .join("kiro"),
            );
        }
    }

    paths
}
// Token 自动刷新后台任务
// 参考 Kiro IDE 源码实现

use crate::commands::common::{
    apply_refreshed_account_tokens, is_auth_error_message, is_token_expired,
    refresh_token_by_provider, token_needs_refresh, REFRESH_LOOP_INTERVAL_SECONDS,
};
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{interval, Duration};

/// Token 刷新服务
pub struct TokenRefreshService {
    app_handle: AppHandle,
}

impl TokenRefreshService {
    /// 创建新的 Token 刷新服务
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }

    /// 启动后台刷新循环
    pub fn start(self) {
        tauri::async_runtime::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(REFRESH_LOOP_INTERVAL_SECONDS));

            loop {
                interval_timer.tick().await;

                if let Err(e) = self.refresh_expiring_tokens().await {
                    log::error!("Token refresh loop error: {}", e);
                }
            }
        });
    }

    /// 检查并刷新即将过期的 token
    async fn refresh_expiring_tokens(&self) -> Result<(), String> {
        // 读取所有账号
        let accounts = {
            let state = self.app_handle.state::<AppState>();
            let mut store = state
                .store
                .lock()
                .map_err(|_| "Failed to acquire account store lock".to_string())?;
            store.reload();
            store.accounts.clone()
        };

        // 本轮是否有账号被刷新或失效，用于决定是否通知前端
        let mut any_changed = false;

        for account in accounts {
            // 跳过已禁用或无效的账号
            if account.status == "invalid" || account.status == "banned" {
                continue;
            }

            // 检查是否需要刷新（即将过期或已过期）
            if let Some(ref expires_at) = account.expires_at {
                if token_needs_refresh(expires_at) {
                    let email_display = account
                        .email
                        .as_deref()
                        .or(account.user_id.as_deref())
                        .unwrap_or("Unknown");

                    log::info!(
                        "Token refresh loop: token expiring soon for account {} ({}), attempting refresh",
                        email_display,
                        account.provider.as_deref().unwrap_or("Unknown")
                    );

                    // 尝试刷新
                    match refresh_token_by_provider(&account).await {
                        Ok(refresh_result) => {
                            // 更新账号信息
                            let state = self.app_handle.state::<AppState>();
                            let mut store = state
                                .store
                                .lock()
                                .map_err(|_| "Failed to acquire account store lock".to_string())?;
                            store.reload();
                            if let Some(acc) = store.accounts.iter_mut().find(|a| a.id == account.id)
                            {
                                if acc.refresh_token.as_deref() != account.refresh_token.as_deref() {
                                    log::info!(
                                        "Token refresh loop: skipped stale refresh result for {}",
                                        email_display
                                    );
                                    continue;
                                }

                                // 先提取 email_display，避免借用冲突
                                let email_display = acc
                                    .email
                                    .as_deref()
                                    .or(acc.user_id.as_deref())
                                    .unwrap_or("Unknown")
                                    .to_string();

                                // 应用刷新结果到 Account（统一的字段更新策略）
                                apply_refreshed_account_tokens(acc, &refresh_result);

                                // 保存到文件
                                if let Err(e) = store.try_save_to_file() {
                                    log::error!("Failed to save account after refresh: {}", e);
                                } else {
                                    any_changed = true;
                                    log::info!(
                                        "Token refresh loop: refresh completed successfully for {}",
                                        email_display
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "Token refresh loop: refresh failed for {}: {}",
                                email_display,
                                e
                            );

                            // 如果是认证错误且 token 已过期，标记为 invalid
                            if is_auth_error_message(&e) && is_token_expired(expires_at) {
                                let state = self.app_handle.state::<AppState>();
                                let mut store = state
                                    .store
                                    .lock()
                                    .map_err(|_| "Failed to acquire account store lock".to_string())?;
                                store.reload();
                                if let Some(acc) = store.accounts.iter_mut().find(|a| a.id == account.id)
                                {
                                    if acc.refresh_token.as_deref() != account.refresh_token.as_deref() {
                                        log::info!(
                                            "Token refresh loop: skipped stale auth failure for {}",
                                            email_display
                                        );
                                        continue;
                                    }

                                    acc.status = "invalid".to_string();
                                    acc.enabled = false;
                                    let _ = store.try_save_to_file();
                                    any_changed = true;
                                    log::warn!(
                                        "Token refresh loop: marked account {} as invalid",
                                        email_display
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // 本轮有任何账号被刷新/失效，通知前端重新加载列表
        if any_changed {
            let _ = self.app_handle.emit("accounts-updated", ());
        }

        Ok(())
    }
}

/// 启动 Token 刷新循环（供 main.rs 调用）
pub fn start_token_refresh_loop(app_handle: AppHandle) {
    log::info!("Starting token refresh background task");
    let service = TokenRefreshService::new(app_handle);
    service.start();
}

// 模型锁定后台任务模块
// 使用 tokio::time::interval 实现真正的后台定时检查

use crate::commands::app_settings_cmd::get_app_settings_inner;
use tauri::AppHandle;
use tokio::time::{interval, Duration};

/// 启动模型锁定后台任务
pub fn start_model_lock_task(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        log::info!("[ModelLock] 后台任务已启动");

        let mut retry_count = 0;
        const MAX_RETRIES: u32 = 3;

        loop {
            // 读取配置
            let settings = match get_app_settings_inner() {
                Ok(s) => {
                    retry_count = 0; // 成功后重置重试计数
                    s
                }
                Err(e) => {
                    retry_count += 1;
                    log::error!(
                        "[ModelLock] 读取配置失败 ({}/{}): {}",
                        retry_count,
                        MAX_RETRIES,
                        e
                    );

                    if retry_count >= MAX_RETRIES {
                        log::error!(
                            "[ModelLock] 达到最大重试次数 ({}), 后台任务停止",
                            MAX_RETRIES
                        );
                        return;
                    }

                    tokio::time::sleep(Duration::from_secs(300)).await;
                    continue;
                }
            };

            // 检查是否启用模型锁定
            if settings.lock_model != Some(true) || settings.locked_model.is_none() {
                log::debug!("[ModelLock] 模型锁定已禁用，等待 30 分钟后重新检查");
                tokio::time::sleep(Duration::from_secs(1800)).await;
                continue;
            }

            let locked_model = settings.locked_model.unwrap();
            log::info!("[ModelLock] 模型锁定已启用，锁定模型: {}", locked_model);

            // 创建定时器（每 30 秒检查一次）
            let mut timer = interval(Duration::from_secs(30));
            // 消耗第一次 tick
            timer.tick().await;

            // 立即检查一次
            check_and_restore_model(&app_handle, &locked_model).await;

            // 定时检查
            loop {
                timer.tick().await;

                // 重新检查配置（用户可能修改了设置）
                let current_settings = match get_app_settings_inner() {
                    Ok(s) => s,
                    Err(_) => break, // 读取失败，退出内层循环，重新初始化
                };

                // 如果禁用了模型锁定，退出内层循环
                if current_settings.lock_model != Some(true)
                    || current_settings.locked_model.is_none()
                {
                    log::info!("[ModelLock] 模型锁定已禁用");
                    break;
                }

                // 如果锁定的模型改变了，退出内层循环，重新初始化
                let current_locked_model = current_settings.locked_model.unwrap();
                if current_locked_model != locked_model {
                    log::info!(
                        "[ModelLock] 锁定模型已改变: {} -> {}",
                        locked_model,
                        current_locked_model
                    );
                    break;
                }

                // 执行检查
                check_and_restore_model(&app_handle, &locked_model).await;
            }
        }
    });
}

/// 检查并恢复锁定的模型
async fn check_and_restore_model(_app_handle: &AppHandle, locked_model: &str) {
    // 获取当前 Kiro 设置
    let kiro_settings = match crate::commands::kiro_settings_cmd::get_kiro_settings().await {
        Ok(s) => s,
        Err(e) => {
            log::error!("[ModelLock] 读取 Kiro 设置失败: {}", e);
            return;
        }
    };

    // 检查当前模型
    let current_model = match &kiro_settings.model_selection {
        Some(model) => model.clone(),
        None => {
            log::debug!("[ModelLock] 未检测到当前模型");
            return;
        }
    };

    // 如果当前模型与锁定模型不一致，恢复锁定模型
    if current_model != locked_model {
        log::info!(
            "[ModelLock] 检测到模型变更: {} -> {}，恢复锁定模型",
            current_model,
            locked_model
        );

        // 调用设置模型命令
        if let Err(e) =
            crate::commands::kiro_settings_cmd::set_kiro_model(locked_model.to_string()).await
        {
            log::error!("[ModelLock] 恢复锁定模型失败: {}", e);
        } else {
            log::info!("[ModelLock] 已恢复锁定模型: {}", locked_model);
        }
    } else {
        log::debug!("[ModelLock] 模型未变更，保持锁定: {}", locked_model);
    }
}

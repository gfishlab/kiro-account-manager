// 认证相关模块

#[allow(clippy::module_inception)]
mod auth;
pub mod auth_social;
pub mod providers;
mod window_navigation;

// 重新导出常用类型
pub use auth::{
    AuthState, DESKTOP_AUTH_API, DesktopRefreshResponse, User, delete_account_desktop,
    refresh_token_desktop,
};
pub use window_navigation::handle_incoming_deep_link;
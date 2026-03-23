//! 用户状态管理
//!
//! 提供全局用户登录状态管理，供 AI Chat 等模块使用。

use std::sync::{Arc, RwLock};

use gpui::{App, Global};

/// 用户信息
#[derive(Clone, Debug)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: i64,
}

/// 团队选项（用于表单下拉）
#[derive(Clone, Debug)]
pub struct TeamOption {
    pub id: String,
    pub name: String,
}

/// 全局用户状态（供跨 crate 访问登录态）
#[derive(Clone, Default)]
pub struct GlobalUserState {
    user: Arc<RwLock<Option<UserInfo>>>,
}

impl Global for GlobalUserState {}

impl GlobalUserState {
    /// 获取当前用户
    pub fn get_user(cx: &App) -> Option<UserInfo> {
        if let Some(state) = cx.try_global::<GlobalUserState>() {
            state.user.read().ok().and_then(|u| u.clone())
        } else {
            None
        }
    }

    /// 是否已登录
    pub fn is_logged_in(cx: &App) -> bool {
        Self::get_user(cx).is_some()
    }

    /// 设置当前用户
    pub fn set_user(user: Option<UserInfo>, cx: &mut App) {
        if !cx.has_global::<GlobalUserState>() {
            cx.set_global(GlobalUserState::default());
        }
        if let Some(state) = cx.try_global::<GlobalUserState>() {
            if let Ok(mut guard) = state.user.write() {
                *guard = user;
            }
        }
    }
}

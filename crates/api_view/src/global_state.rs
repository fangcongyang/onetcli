//! API 全局状态

use crate::models::{ApiProject, ApiEnvironment};
use std::sync::Arc;

/// API 全局状态
pub struct GlobalApiState {
    /// 当前项目配置
    pub project: ApiProject,
    /// 环境变量缓存
    pub environments: Vec<ApiEnvironment>,
}

impl GlobalApiState {
    pub fn new() -> Self {
        Self {
            project: ApiProject::default(),
            environments: Vec::new(),
        }
    }

    /// 获取当前项目
    pub fn get_project(&self) -> Arc<ApiProject> {
        Arc::new(self.project.clone())
    }

    /// 设置当前项目
    pub fn set_project(&mut self, project: ApiProject) {
        self.project = project;
    }
}

impl Default for GlobalApiState {
    fn default() -> Self {
        Self::new()
    }
}

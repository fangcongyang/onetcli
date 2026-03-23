//! 云端 API 客户端接口
//!
//! 提供云端 API 调用接口，供 AI Provider 等模块使用。

use async_trait::async_trait;
use llm_connector::types::ChatRequest;
use thiserror::Error;

use crate::llm::connector::ChatStream;

/// 云 API 错误
#[derive(Error, Debug)]
pub enum CloudApiError {
    #[error("云 API 请求失败: {0}")]
    RequestFailed(String),
    #[error("未认证")]
    NotAuthenticated,
    #[error("网络错误: {0}")]
    NetworkError(String),
    #[error("服务器错误: {0}")]
    ServerError(String),
    #[error("解析错误: {0}")]
    ParseError(String),
    #[error("API 限流")]
    RateLimited,
}

impl CloudApiError {
    /// 判断是否为认证错误
    pub fn is_auth_error(&self) -> bool {
        matches!(self, CloudApiError::NotAuthenticated)
    }
}

/// 云端 API 客户端 trait
#[async_trait]
pub trait CloudApiClient: Send + Sync {
    /// 发送聊天请求
    async fn chat(&self, request: &ChatRequest) -> Result<String, CloudApiError>;

    /// 发送流式聊天请求
    async fn chat_stream(
        &self,
        request: &ChatRequest,
    ) -> Result<ChatStream, CloudApiError>;

    /// 获取可用模型列表
    async fn list_models(&self) -> Result<Vec<String>, CloudApiError>;
}

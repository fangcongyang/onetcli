//! API 测试数据模型

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// HTTP 方法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
    HEAD,
    OPTIONS,
}

impl Default for HttpMethod {
    fn default() -> Self {
        HttpMethod::GET
    }
}

impl HttpMethod {
    pub fn all() -> Vec<HttpMethod> {
        vec![
            HttpMethod::GET,
            HttpMethod::POST,
            HttpMethod::PUT,
            HttpMethod::DELETE,
            HttpMethod::PATCH,
            HttpMethod::HEAD,
            HttpMethod::OPTIONS,
        ]
    }
}

/// 认证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    None,
    Basic {
        username: String,
        password: String,
    },
    Bearer {
        token: String,
    },
    ApiKey {
        key: String,
        value: String,
        add_to: ApiKeyAddTo,
    },
}

/// API Key 添加位置
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyAddTo {
    Header,
    Query,
}

/// 请求体类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequestBody {
    None,
    FormData {
        fields: Vec<FormField>,
    },
    Urlencoded {
        fields: Vec<FormField>,
    },
    Raw {
        content_type: String,
        content: String,
    },
    Binary {
        file_path: String,
    },
}

impl Default for RequestBody {
    fn default() -> Self {
        RequestBody::None
    }
}

/// 表单字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub key: String,
    pub value: String,
    pub is_file: bool,
    pub enabled: bool,
}

/// 单个 API 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    pub id: Uuid,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub params: Vec<KeyValue>,
    pub headers: Vec<KeyValue>,
    pub body: RequestBody,
    pub auth: AuthConfig,
    pub enabled: bool,
}

impl ApiRequest {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            method: HttpMethod::GET,
            url: String::new(),
            params: Vec::new(),
            headers: Vec::new(),
            body: RequestBody::default(),
            auth: AuthConfig::None,
            enabled: true,
        }
    }
}

/// Key-Value 对（用于 params 和 headers）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    pub enabled: bool,
    pub description: Option<String>,
}

impl KeyValue {
    pub fn new(key: String, value: String) -> Self {
        Self {
            key,
            value,
            enabled: true,
            description: None,
        }
    }
}

/// 环境变量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEnvironment {
    pub id: Uuid,
    pub name: String,
    pub variables: Vec<EnvironmentVariable>,
}

impl ApiEnvironment {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            variables: Vec::new(),
        }
    }
}

/// 环境变量
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentVariable {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

/// 请求集合
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCollection {
    pub id: Uuid,
    pub name: String,
    pub requests: Vec<ApiRequest>,
    pub folders: Vec<ApiCollection>,
}

impl ApiCollection {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            requests: Vec::new(),
            folders: Vec::new(),
        }
    }
}

/// API 项目配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiProject {
    pub collections: Vec<ApiCollection>,
    pub environments: Vec<ApiEnvironment>,
    pub active_env_id: Option<Uuid>,
    pub history: Vec<HistoryEntry>,
}

/// 历史记录条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: Uuid,
    pub request: ApiRequest,
    pub response: Option<ResponseSummary>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 响应摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseSummary {
    pub status: u16,
    pub status_text: String,
    pub time_ms: u64,
    pub size_bytes: usize,
}

impl Default for ApiProject {
    fn default() -> Self {
        Self {
            collections: Vec::new(),
            environments: Vec::new(),
            active_env_id: None,
            history: Vec::new(),
        }
    }
}

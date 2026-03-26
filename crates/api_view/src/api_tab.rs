//! API 测试标签页视图

use crate::collections_storage::CollectionsStorage;
use crate::history_storage::HistoryStorage;
use crate::models::{ApiCollection, ApiRequest, HttpMethod, KeyValue};
use futures::AsyncReadExt;
use gpui::prelude::FluentBuilder;
use gpui::{
    http_client::{self, AsyncBody, Url},
    App, AppContext, AsyncApp, ClickEvent, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, PathPromptOptions, Pixels,
    Render, SharedString, Styled, Window, div, px,
};
use serde::{Deserialize, Serialize};
use gpui_component::input::InputState;
use gpui_component::select::{Select, SelectState, SelectEvent, SearchableVec, SelectItem};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::checkbox::Checkbox;
use gpui_component::{
    scroll::ScrollableElement, ActiveTheme, Icon, IconName, IndexPath, Sizable, Size, StyledExt, h_flex, v_flex,
};
use gpui_component::resizable::{h_resizable, resizable_panel, ResizableState};
use one_core::tab_container::{TabContent, TabContentEvent};

const REQUEST_PANEL_MIN_WIDTH: Pixels = px(400.0);
const SIDEBAR_WIDTH: Pixels = px(240.0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    Params,
    Body,
    Headers,
    Auth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyType {
    Json,
    FormData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormDataValueType {
    Text,
    File,
}

#[derive(Clone)]
struct FormDataValueTypeItem {
    value_type: FormDataValueType,
    label: String,
}

impl FormDataValueTypeItem {
    fn new(value_type: FormDataValueType, label: &str) -> Self {
        Self { value_type, label: label.to_string() }
    }
}

impl SelectItem for FormDataValueTypeItem {
    type Value = FormDataValueType;

    fn title(&self) -> SharedString {
        SharedString::from(self.label.clone())
    }

    fn value(&self) -> &Self::Value {
        &self.value_type
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().text_sm().child(self.label.clone())
    }
}

#[derive(Clone)]
struct FormDataRow {
    key_input: Entity<InputState>,
    value_input: Entity<InputState>,
    value_type: FormDataValueType,
    file_path: Option<String>,
    enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponsePanel {
    Body,
    Headers,
    Cookies,
}

fn method_color(method: &str) -> gpui::Hsla {
    match method {
        "GET" => gpui::Hsla::green(),
        "POST" => gpui::Hsla::blue(),
        "PUT" => gpui::Hsla { h: 0.08, s: 0.9, l: 0.5, a: 1.0 },      // 橙色
        "DELETE" => gpui::Hsla::red(),
        "PATCH" => gpui::Hsla { h: 0.75, s: 0.7, l: 0.5, a: 1.0 },   // 紫色
        "HEAD" => gpui::Hsla { h: 0.0, s: 0.0, l: 0.5, a: 1.0 },     // 灰色
        "OPTIONS" => gpui::Hsla { h: 0.5, s: 0.8, l: 0.5, a: 1.0 },  // 青色
        _ => gpui::Hsla { h: 0.0, s: 0.0, l: 0.5, a: 1.0 },
    }
}

#[derive(Clone)]
struct HttpMethodItem {
    method: String,
    color: gpui::Hsla,
}

impl HttpMethodItem {
    fn new(method: &str) -> Self {
        let color = method_color(method);
        Self {
            method: method.to_string(),
            color,
        }
    }
}

impl SelectItem for HttpMethodItem {
    type Value = String;

    fn title(&self) -> SharedString {
        SharedString::from(self.method.clone())
    }

    fn value(&self) -> &Self::Value {
        &self.method
    }

    fn display_title(&self) -> Option<gpui::AnyElement> {
        Some(
            div()
                .text_xs()
                .font_bold()
                .text_color(self.color)
                .child(self.method.clone())
                .into_any_element()
        )
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        h_flex()
            .gap_2()
            .items_center()
            .child(
                div()
                    .text_xs()
                    .font_bold()
                    .text_color(self.color)
                    .child(self.method.clone())
            )
    }
}

#[derive(Clone)]
struct BodyTypeItem {
    body_type: BodyType,
    label: String,
}

impl BodyTypeItem {
    fn new(body_type: BodyType, label: &str) -> Self {
        Self {
            body_type,
            label: label.to_string(),
        }
    }
}

impl SelectItem for BodyTypeItem {
    type Value = BodyType;

    fn title(&self) -> SharedString {
        SharedString::from(self.label.clone())
    }

    fn value(&self) -> &Self::Value {
        &self.body_type
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .text_sm()
            .child(self.label.clone())
    }
}

#[derive(Debug, Clone)]
pub struct ResponseStatus {
    pub status: u16,
    pub status_text: String,
    pub time_ms: u64,
    pub body: String,
    pub headers: Vec<(String, String)>,
    pub cookies: Vec<(String, String)>,
}

pub struct ApiTabView {
    current_request: ApiRequest,
    response_status: Option<ResponseStatus>,
    is_loading: bool,
    active_panel: ActivePanel,
    active_response_tab: ResponsePanel,
    focus_handle: FocusHandle,
    project_id: String,
    
    // URL 输入框
    url_input: Entity<InputState>,
    
    // HTTP 方法选择器
    method_select: Entity<SelectState<SearchableVec<HttpMethodItem>>>,
    
    // Params 编辑器
    params: Vec<KeyValueRow>,
    
    // Headers 编辑器
    headers: Vec<KeyValueRow>,
    
    // Body 类型
    body_type: BodyType,
    body_type_select: Entity<SelectState<SearchableVec<BodyTypeItem>>>,
    
    // Body 输入
    body_input: Entity<InputState>,
    
    // Form Data 表单字段
    body_form_fields: Vec<FormDataRow>,
    
    // 请求历史
    history: Vec<HistoryItem>,
    
    // Collections
    collections: Vec<ApiCollection>,
    
    // 待处理的 Content-Type 更新
    pending_content_type_update: bool,
    
    // 可拖动分栏状态
    resizable_state: Entity<ResizableState>,
    sidebar_resizable_state: Entity<ResizableState>,
    
    // 侧边栏内部折叠状态
    collections_collapsed: bool,
    history_collapsed: bool,
    
    // 响应 Body 输入框
    response_body_input: Option<Entity<InputState>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub id: String,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub timestamp: i64,
    pub response_time_ms: u64,
}

struct KeyValueRow {
    key_input: Entity<InputState>,
    value_input: Entity<InputState>,
    enabled: bool,
    default_key: Option<String>,
    default_value: Option<String>,
}

impl KeyValueRow {
    fn new(window: &mut Window, cx: &mut Context<ApiTabView>) -> Self {
        Self {
            key_input: cx.new(|cx| InputState::new(window, cx).placeholder("Key")),
            value_input: cx.new(|cx| InputState::new(window, cx).placeholder("Value")),
            enabled: true,
            default_key: None,
            default_value: None,
        }
    }
    
    fn with_defaults(key: &str, value: &str, window: &mut Window, cx: &mut Context<ApiTabView>) -> Self {
        let key_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Key")
        });
        let value_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Value")
        });
        Self {
            key_input,
            value_input,
            enabled: true,
            default_key: Some(key.to_string()),
            default_value: Some(value.to_string()),
        }
    }
    
    fn get_display_key(&self, cx: &App) -> String {
        let text = self.key_input.read(cx).text().to_string();
        if text.is_empty() {
            self.default_key.clone().unwrap_or_default()
        } else {
            text
        }
    }
    
    fn get_display_value(&self, cx: &App) -> String {
        let text = self.value_input.read(cx).text().to_string();
        if text.is_empty() {
            self.default_value.clone().unwrap_or_default()
        } else {
            text
        }
    }
}

impl FormDataRow {
    fn new(window: &mut Window, cx: &mut Context<ApiTabView>) -> Self {
        Self {
            key_input: cx.new(|cx| InputState::new(window, cx)),
            value_input: cx.new(|cx| InputState::new(window, cx)),
            value_type: FormDataValueType::Text,
            file_path: None,
            enabled: true,
        }
    }
}

impl ApiTabView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_project_id("default".to_string(), window, cx)
    }

    pub fn with_project_id(project_id: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 加载历史记录
        let history = HistoryStorage::load();
        
        // 加载 Collections
        let collections = CollectionsStorage::load();
        
        // 创建 URL 输入框
        let url_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Enter request URL")
        });

        // 创建 HTTP 方法选择器
        let methods = vec![
            HttpMethodItem::new("GET"),
            HttpMethodItem::new("POST"),
            HttpMethodItem::new("PUT"),
            HttpMethodItem::new("DELETE"),
            HttpMethodItem::new("PATCH"),
            HttpMethodItem::new("HEAD"),
            HttpMethodItem::new("OPTIONS"),
        ];
        let method_select = cx.new(|cx| {
            SelectState::new(SearchableVec::new(methods), None, window, cx)
        });

        // 设置默认选中的方法
        method_select.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(0)), window, cx);
        });

        // 创建 Body 类型选择器
        let body_types = vec![
            BodyTypeItem::new(BodyType::Json, "JSON"),
            BodyTypeItem::new(BodyType::FormData, "Form Data"),
        ];
        let body_type_select = cx.new(|cx| {
            SelectState::new(SearchableVec::new(body_types), None, window, cx)
        });
        body_type_select.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(0)), window, cx);
        });

        // 创建初始 Params 行
        let params = vec![KeyValueRow::new(window, cx)];

        // 创建初始 Headers 行，包含默认 User-Agent header
        let headers = vec![
            KeyValueRow::with_defaults("User-Agent", "OneHub/1.0", window, cx),
            KeyValueRow::new(window, cx),
        ];

        let resizable_state = cx.new(|cx| {
            ResizableState::with_sizes(vec![px(400.0), px(400.0)])
        });
        let sidebar_resizable_state = cx.new(|cx| {
            ResizableState::with_sizes(vec![px(240.0), px(800.0)])
        });
        
        let mut view = Self {
            current_request: ApiRequest::new("New Request".to_string()),
            response_status: None,
            is_loading: false,
            active_panel: ActivePanel::Params,
            active_response_tab: ResponsePanel::Body,
            focus_handle: cx.focus_handle(),
            project_id,
            url_input,
            method_select,
            params,
            headers,
            body_type: BodyType::Json,
            body_type_select,
            body_input: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("Request body (JSON)")
                    .code_editor("json")
            }),
            body_form_fields: vec![FormDataRow::new(window, cx)],
            history,
            collections,
            resizable_state,
            sidebar_resizable_state,
            collections_collapsed: false,
            history_collapsed: false,
            response_body_input: None,
            pending_content_type_update: false,
        };

        // 设置事件订阅
        view.setup_subscriptions(window, cx);

        view
    }

    fn setup_subscriptions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let url_input = self.url_input.clone();
        cx.subscribe(&url_input, |this, input, event, cx| {
            use gpui_component::input::InputEvent;
            if let InputEvent::Change = event {
                this.current_request.url = input.read(cx).text().to_string();
            }
        }).detach();

        let method_select = self.method_select.clone();
        cx.subscribe(&method_select, |this, _, event, _cx| {
            if let SelectEvent::Confirm(Some(value)) = event {
                this.current_request.method = Self::parse_method(value.value());
            }
        }).detach();

        let body_type_select = self.body_type_select.clone();
        cx.subscribe(&body_type_select, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(value)) = event {
                this.body_type = *value;
                this.pending_content_type_update = true;
                cx.notify();
            }
        }).detach();
    }

    fn parse_method(method_str: &str) -> HttpMethod {
        match method_str {
            "GET" => HttpMethod::GET,
            "POST" => HttpMethod::POST,
            "PUT" => HttpMethod::PUT,
            "DELETE" => HttpMethod::DELETE,
            "PATCH" => HttpMethod::PATCH,
            "HEAD" => HttpMethod::HEAD,
            "OPTIONS" => HttpMethod::OPTIONS,
            _ => HttpMethod::GET,
        }
    }

    fn method_to_string(method: HttpMethod) -> String {
        match method {
            HttpMethod::GET => "GET".to_string(),
            HttpMethod::POST => "POST".to_string(),
            HttpMethod::PUT => "PUT".to_string(),
            HttpMethod::DELETE => "DELETE".to_string(),
            HttpMethod::PATCH => "PATCH".to_string(),
            HttpMethod::HEAD => "HEAD".to_string(),
            HttpMethod::OPTIONS => "OPTIONS".to_string(),
        }
    }

    fn format_header_key(key: &str) -> String {
        let mut result = String::new();
        let mut capitalize_next = true;
        
        for c in key.chars() {
            if c == '-' || c == '_' {
                result.push(' ');
                capitalize_next = true;
            } else if capitalize_next {
                result.extend(c.to_uppercase());
                capitalize_next = false;
            } else {
                result.push(c);
            }
        }
        result
    }

    pub fn send_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_loading {
            return;
        }

        // 收集 Params
        self.current_request.params = self.params.iter().filter_map(|row| {
            let key = row.key_input.read(cx).text().to_string();
            let value = row.value_input.read(cx).text().to_string();
            if !key.is_empty() {
                Some(KeyValue { key, value, enabled: row.enabled, description: None })
            } else {
                None
            }
        }).collect();

        // 收集 Headers
        self.current_request.headers = self.headers.iter().filter_map(|row| {
            let key = row.key_input.read(cx).text().to_string();
            let value = row.value_input.read(cx).text().to_string();
            if !key.is_empty() {
                Some(KeyValue { key, value, enabled: row.enabled, description: None })
            } else {
                None
            }
        }).collect();

        // 收集 Body
        self.current_request.body = match self.body_type {
            BodyType::Json => {
                let content = self.body_input.read(cx).text().to_string();
                if content.trim().is_empty() {
                    crate::models::RequestBody::None
                } else {
                    crate::models::RequestBody::Raw {
                        content_type: "application/json".to_string(),
                        content,
                    }
                }
            }
            BodyType::FormData => {
                let fields: Vec<crate::models::FormField> = self.body_form_fields.iter()
                    .filter(|row| row.enabled)
                    .filter_map(|row| {
                        let key = row.key_input.read(cx).text().to_string();
                        if key.is_empty() {
                            return None;
                        }
                        let (value, is_file) = if row.value_type == FormDataValueType::File {
                            (row.file_path.clone().unwrap_or_default(), true)
                        } else {
                            (row.value_input.read(cx).text().to_string(), false)
                        };
                        Some(crate::models::FormField { key, value, is_file, enabled: true })
                    })
                    .collect();
                if fields.is_empty() {
                    crate::models::RequestBody::None
                } else {
                    crate::models::RequestBody::FormData { fields }
                }
            }
        };

        self.is_loading = true;
        cx.notify();

        let request = self.current_request.clone();
        let this = cx.entity().clone();
        let method_str = Self::method_to_string(self.current_request.method.clone());
        let url = self.current_request.url.clone();

        cx.spawn(async move |_weak_entity, cx: &mut AsyncApp| {
            let result = Self::execute_request(request).await;
            
            this.update(cx, |this, cx| {
                this.response_status = result.clone();
                this.is_loading = false;
                
                // 添加到历史记录并保存
                if let Some(ref resp) = result {
                    let history_item = HistoryItem {
                        id: uuid::Uuid::new_v4().to_string(),
                        method: method_str,
                        url: url.clone(),
                        status: resp.status,
                        timestamp: chrono::Utc::now().timestamp(),
                        response_time_ms: resp.time_ms,
                    };
                    this.history.insert(0, history_item);
                    // 限制历史记录数量
                    if this.history.len() > 50 {
                        this.history.pop();
                    }
                    // 保存到文件
                    HistoryStorage::save(&this.history);
                }
                
                cx.notify();
            });
        })
        .detach();
    }

    async fn execute_request(request: ApiRequest) -> Option<ResponseStatus> {
        use gpui::http_client::HttpClient;
        use reqwest_client::ReqwestClient;
        
        let client = ReqwestClient::new();
        
        if request.url.is_empty() {
            return None;
        }
        
        let mut url = request.url.clone();
        
        let params: Vec<(String, String)> = request.params
            .iter()
            .filter(|p| p.enabled && !p.key.is_empty())
            .map(|p| (p.key.clone(), p.value.clone()))
            .collect();
        
        if !params.is_empty() {
            let query_string = params
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&");
            if url.contains('?') {
                url = format!("{}&{}", url, query_string);
            } else {
                url = format!("{}?{}", url, query_string);
            }
        }

        let parsed_url = Url::parse(&url).ok()?;
        
        let method = match request.method {
            HttpMethod::GET => http_client::http::Method::GET,
            HttpMethod::POST => http_client::http::Method::POST,
            HttpMethod::PUT => http_client::http::Method::PUT,
            HttpMethod::DELETE => http_client::http::Method::DELETE,
            HttpMethod::PATCH => http_client::http::Method::PATCH,
            HttpMethod::HEAD => http_client::http::Method::HEAD,
            HttpMethod::OPTIONS => http_client::http::Method::OPTIONS,
        };

        let mut builder = http_client::http::Request::builder()
            .uri(parsed_url.as_str())
            .method(method);

        for header in &request.headers {
            if header.enabled && !header.key.is_empty() {
                builder = builder.header(&header.key, &header.value);
            }
        }

        match &request.auth {
            crate::models::AuthConfig::Basic { username, password } => {
                let encoded = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    format!("{}:{}", username, password)
                );
                builder = builder.header("Authorization", format!("Basic {}", encoded));
            }
            crate::models::AuthConfig::Bearer { token } => {
                builder = builder.header("Authorization", format!("Bearer {}", token));
            }
            _ => {}
        }

        // 处理请求体
        let req: http_client::http::Request<AsyncBody> = match &request.body {
            crate::models::RequestBody::None => {
                builder.body(AsyncBody::empty()).ok()?
            }
            crate::models::RequestBody::Raw { content_type, content } => {
                builder
                    .header("Content-Type", content_type)
                    .body(AsyncBody::from(content.clone().into_bytes()))
                    .ok()?
            }
            crate::models::RequestBody::FormData { fields } => {
                let boundary = uuid::Uuid::new_v4().to_string();
                let mut body_bytes = Vec::new();
                for field in fields {
                    if !field.enabled || field.key.is_empty() {
                        continue;
                    }
                    body_bytes.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
                    
                    if field.is_file && !field.value.is_empty() {
                        // 文件字段
                        let filename = std::path::Path::new(&field.value)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| field.value.clone());
                        body_bytes.extend_from_slice(format!("Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n", field.key, filename).as_bytes());
                        body_bytes.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
                        if let Ok(file_content) = std::fs::read(&field.value) {
                            body_bytes.extend_from_slice(&file_content);
                        }
                    } else {
                        // 文本字段
                        body_bytes.extend_from_slice(format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", field.key).as_bytes());
                        body_bytes.extend_from_slice(field.value.as_bytes());
                    }
                    body_bytes.extend_from_slice(b"\r\n");
                }
                body_bytes.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
                
                builder
                    .header("Content-Type", format!("multipart/form-data; boundary={}", boundary))
                    .body(AsyncBody::from(body_bytes))
                    .ok()?
            }
            crate::models::RequestBody::Urlencoded { fields } => {
                let mut form_data = Vec::new();
                for field in fields {
                    if !field.enabled || field.key.is_empty() {
                        continue;
                    }
                    if !form_data.is_empty() {
                        form_data.push(b'&');
                    }
                    form_data.extend_from_slice(format!("{}={}",
                        urlencoding::encode(&field.key),
                        urlencoding::encode(&field.value)
                    ).as_bytes());
                }
                builder
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(AsyncBody::from(form_data))
                    .ok()?
            }
            crate::models::RequestBody::Binary { file_path } => {
                if let Ok(content) = std::fs::read(file_path) {
                    builder
                        .header("Content-Type", "application/octet-stream")
                        .body(AsyncBody::from(content))
                        .ok()?
                } else {
                    builder.body(AsyncBody::empty()).ok()?
                }
            }
        };

        let start = std::time::Instant::now();
        let response = client.send(req).await.ok()?;
        let time_ms = start.elapsed().as_millis() as u64;

        let status = response.status();
        let status_text = status.canonical_reason().unwrap_or("Unknown").to_string();

        // 收集响应头
        let headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        // 获取响应体
        let mut body_stream = response.into_body();
        let mut body_bytes = Vec::new();
        body_stream.read_to_end(&mut body_bytes).await.ok();
        let body = String::from_utf8_lossy(&body_bytes).to_string();

        Some(ResponseStatus {
            status: status.as_u16(),
            status_text: status_text.clone(),
            time_ms,
            body,
            headers,
            cookies: Vec::new(),
        })
    }

    fn handle_send_click(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.send_request(window, cx);
    }

    fn handle_add_param(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.params.push(KeyValueRow::new(window, cx));
        cx.notify();
    }

    fn handle_add_header(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.headers.push(KeyValueRow::new(window, cx));
        cx.notify();
    }

    fn handle_delete_param(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.params.len() {
            self.params.remove(index);
            cx.notify();
        }
    }

    fn handle_delete_header(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.headers.len() {
            self.headers.remove(index);
            cx.notify();
        }
    }

    fn handle_panel_tab_click(&mut self, panel: ActivePanel, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.active_panel = panel;
        cx.notify();
    }

    fn handle_new_request(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        // 清空 URL 输入框
        self.url_input.update(cx, |input, cx| {
            input.set_value("".to_string(), window, cx);
        });
        
        // 重置 Params
        self.params.clear();
        self.params.push(KeyValueRow::new(window, cx));
        
        // 重置 Headers
        self.headers.clear();
        self.headers.push(KeyValueRow::with_defaults("User-Agent", "OneHub/1.0", window, cx));
        self.headers.push(KeyValueRow::new(window, cx));
        
        self.current_request = ApiRequest::new("New Request".to_string());
        self.response_status = None;
        self.active_panel = ActivePanel::Params;
        cx.notify();
    }

    fn handle_save_to_collection(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let url = self.url_input.read(cx).text().to_string();
        
        // 获取当前选中的方法
        let method_str = self.method_select.read(cx)
            .selected_index(cx)
            .map(|ix| {
                match ix.section {
                    0 => "GET",
                    1 => "POST",
                    2 => "PUT",
                    3 => "DELETE",
                    4 => "PATCH",
                    5 => "HEAD",
                    6 => "OPTIONS",
                    _ => "GET",
                }
            })
            .unwrap_or("GET");
        
        // 创建新的 ApiRequest
        let mut request = ApiRequest::new(format!("{} - {}", method_str, url));
        request.method = match method_str {
            "GET" => HttpMethod::GET,
            "POST" => HttpMethod::POST,
            "PUT" => HttpMethod::PUT,
            "DELETE" => HttpMethod::DELETE,
            "PATCH" => HttpMethod::PATCH,
            _ => HttpMethod::GET,
        };
        request.url = url;
        
        // 如果没有 collection，创建一个默认的
        if self.collections.is_empty() {
            let default_collection = ApiCollection::new("My Collection".to_string());
            self.collections.push(default_collection);
        }
        
        // 添加请求到第一个 collection
        self.collections[0].requests.push(request);
        
        // 保存到存储
        CollectionsStorage::save(&self.collections);
        
        cx.notify();
    }

    fn handle_new_collection(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let new_collection = ApiCollection::new("New Collection".to_string());
        self.collections.push(new_collection);
        CollectionsStorage::save(&self.collections);
        cx.notify();
    }

    fn handle_toggle_collections(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.collections_collapsed = !self.collections_collapsed;
        cx.notify();
    }

    fn handle_toggle_history(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.history_collapsed = !self.history_collapsed;
        cx.notify();
    }

    fn load_history_item(&mut self, item: &HistoryItem, window: &mut Window, cx: &mut Context<Self>) {
        // 更新 URL
        self.url_input.update(cx, |input, cx| {
            input.set_value(item.url.clone(), window, cx);
        });
        
        // 更新方法选择器
        if let Some(index) = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"]
            .iter()
            .position(|&m| m == item.method)
        {
            self.method_select.update(cx, |state, cx| {
                state.set_selected_index(Some(IndexPath::new(index)), window, cx);
            });
        }
        
        cx.notify();
    }

    fn load_request_from_collection(&mut self, request: &ApiRequest, window: &mut Window, cx: &mut Context<Self>) {
        // 更新 URL
        self.url_input.update(cx, |input, cx| {
            input.set_value(request.url.clone(), window, cx);
        });
        
        // 更新方法选择器
        let method_str = Self::method_to_string(request.method.clone());
        if let Some(index) = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"]
            .iter()
            .position(|m| *m == method_str)
        {
            self.method_select.update(cx, |state, cx| {
                state.set_selected_index(Some(IndexPath::new(index)), window, cx);
            });
        }
        
        cx.notify();
    }
}

impl Render for ApiTabView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let resizable_state = self.resizable_state.clone();
        
        h_flex()
            .key_context("ApiTab")
            .size_full()
            .bg(cx.theme().background)
            .track_focus(&self.focus_handle)
            .child(
                div()
                    .w(px(240.0))
                    .h_full()
                    .child(self.render_sidebar(cx)),
            )
            .child(
                div()
                    .w(px(1.0))
                    .bg(cx.theme().border),
            )
            .child(
                h_resizable("api-split")
                    .with_state(&resizable_state)
                    .on_resize(|_, _, _| {})
                    .child(
                        resizable_panel()
                            .size(px(400.0))
                            .size_range(px(200.0)..px(800.0))
                            .child(
                                v_flex()
                                    .size_full()
                                    .child(self.render_url_bar(cx))
                                    .child(self.render_tab_bar(cx))
                                    .child(self.render_request_panel(window, cx)),
                            ),
                    )
                    .child(
                        resizable_panel()
                            .size(px(400.0))
                            .size_range(px(200.0)..px(800.0))
                            .child(self.render_response_panel(window, cx)),
                    ),
            )
    }
}

impl ApiTabView {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .child(
                v_flex()
                    .w_full()
                    .p_3()
                    .gap_2()
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("toggle-collections")
                                    .icon(if self.collections_collapsed {
                                        IconName::ChevronRight
                                    } else {
                                        IconName::ChevronDown
                                    })
                                    .ghost()
                                    .on_click(cx.listener(Self::handle_toggle_collections)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .font_semibold()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Collections"),
                            )
                            .child(
                                Button::new("new-collection")
                                    .icon(IconName::Plus)
                                    .ghost()
                                    .tooltip("New Collection")
                                    .on_click(cx.listener(Self::handle_new_collection)),
                            )
                            .child(
                                Button::new("save-to-collection")
                                    .icon(IconName::Folder)
                                    .ghost()
                                    .tooltip("Save to Collection")
                                    .on_click(cx.listener(Self::handle_save_to_collection)),
                            ),
                    ),
            )
                            .when(!self.collections_collapsed, |this| {
                                this.child(
                                    div()
                                        .flex_1()
                                        .overflow_y_scrollbar()
                                        .children(self.collections.iter().enumerate().map(|(col_idx, collection)| {
                                            let col_idx = col_idx;
                                            v_flex()
                                                .gap_1()
                                                .children(collection.requests.iter().enumerate().map(move |(req_idx, request)| {
                                                    div()
                                                                .id(("collection", col_idx))
                                                        .child(
                                                            h_flex()
                                                                .gap_2()
                                                                .items_center()
                                                                .child(
                                                                    div()
                                                                        .px_2()
                                                                        .py_1()
                                                                        .rounded_sm()
                                                                        .bg(method_color(&Self::method_to_string(request.method)))
                                                                        .text_xs()
                                                                        .font_bold()
                                                                        .text_color(gpui::white())
                                                                        .child(Self::method_to_string(request.method.clone()))
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .text_xs()
                                                                        .truncate()
                                                                        .child(request.name.clone())
                                                                )
                                                        )
                                                }))
                                        }))
                        .when(self.collections.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("No collections")
                            )
                        }),
                )
            })
            .child(div().w_full().h_px().bg(cx.theme().border))
            .child(
                v_flex()
                    .flex_1()
                    .w_full()
                    .p_3()
                    .gap_2()
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("HISTORY"),
                            )
                            .child(
                                Button::new("toggle-history")
                                    .icon(if self.history_collapsed {
                                        IconName::ChevronRight
                                    } else {
                                        IconName::ChevronDown
                                    })
                                    .ghost()
                                    .on_click(cx.listener(Self::handle_toggle_history)),
                            ),
                    )
                    .when(!self.history_collapsed, |this| {
                        this.child(
                            div()
                                .flex_1()
                                .overflow_y_scrollbar()
                                .children(self.history.iter().enumerate().map(|(index, item)| {
                                    let item_clone = item.clone();
                                    Button::new(("history", index))
                                        .ghost()
                                        .w_full()
                                        .on_click(cx.listener(move |this, _event, window, cx| {
                                            this.load_history_item(&item_clone, window, cx);
                                        }))
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .px_2()
                                                        .py_1()
                                                        .rounded_sm()
                                                        .bg(method_color(&item.method))
                                                        .text_xs()
                                                        .font_bold()
                                                        .text_color(gpui::white())
                                                        .child(item.method.clone())
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .text_xs()
                                                        .overflow_hidden()
                                                        .child(
                                                            div()
                                                                .truncate()
                                                                .child(item.url.clone())
                                                        )
                                                )
                                        )
                                }))
                                .when(self.history.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("No request history")
                                    )
                                }),
                        )
                    }),
            )
    }

    fn render_url_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_loading = self.is_loading;

        h_flex()
            .px_4()
            .py_3()
            .gap_2()
            .items_center()
            .bg(cx.theme().background)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Select::new(&self.method_select)
                    .with_size(Size::Small)
                    .w(px(100.0)),
            )
            .child(
                gpui_component::input::Input::new(&self.url_input)
                    .w(px(900.0))
                    .with_size(Size::Small),
            )
            .child(
                Button::new("send-btn")
                    .with_size(Size::Small)
                    .primary()
                    .loading(is_loading)
                    .label("Send")
                    .on_click(cx.listener(Self::handle_send_click)),
            )
    }

    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .px_4()
            .py_2()
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .children([
                ("Params", ActivePanel::Params),
                ("Body", ActivePanel::Body),
                ("Headers", ActivePanel::Headers),
                ("Auth", ActivePanel::Auth),
            ].map(|(label, panel)| {
                let is_active = self.active_panel == panel;
                if is_active {
                    Button::new(label)
                        .with_size(Size::Small)
                        .primary()
                        .label(label)
                        .on_click(cx.listener(move |this, event, window, cx| {
                            this.handle_panel_tab_click(panel, event, window, cx);
                        }))
                } else {
                    Button::new(label)
                        .with_size(Size::Small)
                        .ghost()
                        .label(label)
                        .on_click(cx.listener(move |this, event, window, cx| {
                            this.handle_panel_tab_click(panel, event, window, cx);
                        }))
                }
            }))
    }

    fn render_request_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .min_w(REQUEST_PANEL_MIN_WIDTH)
            .p_4()
            .child(match self.active_panel {
                ActivePanel::Params => self.render_params_panel(cx).into_any_element(),
                ActivePanel::Body => self.render_body_panel(window, cx).into_any_element(),
                ActivePanel::Headers => self.render_headers_panel(cx).into_any_element(),
                ActivePanel::Auth => self.render_auth_panel(cx).into_any_element(),
            })
    }

    fn render_params_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .children(self.params.iter().enumerate().map(|(index, param)| {
                let key_input = param.key_input.clone();
                let value_input = param.value_input.clone();
                let enabled = param.enabled;
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Checkbox::new(("param-enabled", index))
                            .checked(enabled)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if index < this.params.len() {
                                    this.params[index].enabled = !this.params[index].enabled;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        gpui_component::input::Input::new(&key_input)
                            .flex_1(),
                    )
                    .child(
                        gpui_component::input::Input::new(&value_input)
                            .flex_1(),
                    )
                    .child(
                        Button::new(("param-delete", index))
                            .with_size(Size::Small)
                            .ghost()
                            .icon(IconName::Delete)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if index < this.params.len() {
                                    this.params.remove(index);
                                    cx.notify();
                                }
                            })),
                    )
            }))
            .child(
                Button::new("add-param")
                    .with_size(Size::Small)
                    .ghost()
                    .label("+ Add Parameter")
                    .on_click(cx.listener(Self::handle_add_param)),
            )
    }

    fn render_headers_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .children(self.headers.iter().enumerate().map(|(index, header)| {
                let key_input = header.key_input.clone();
                let value_input = header.value_input.clone();
                let enabled = header.enabled;
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Checkbox::new(("header-enabled", index))
                            .checked(enabled)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if index < this.headers.len() {
                                    this.headers[index].enabled = !this.headers[index].enabled;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        gpui_component::input::Input::new(&key_input)
                            .flex_1(),
                    )
                    .child(
                        gpui_component::input::Input::new(&value_input)
                            .flex_1(),
                    )
                    .child(
                        Button::new(("header-delete", index))
                            .with_size(Size::Small)
                            .ghost()
                            .icon(IconName::Delete)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if index < this.headers.len() {
                                    this.headers.remove(index);
                                    cx.notify();
                                }
                            })),
                    )
            }))
            .child(
                Button::new("add-header")
                    .with_size(Size::Small)
                    .ghost()
                    .label("+ Add Header")
                    .on_click(cx.listener(Self::handle_add_header)),
            )
    }

    fn render_body_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body_input = self.body_input.clone();
        let body_type_select = self.body_type_select.clone();
        
        if self.pending_content_type_update {
            self.pending_content_type_update = false;
            self.update_content_type_header(window, cx);
        }
        
        v_flex()
            .flex_1()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Body Type:"),
                    )
                    .child(
                        Select::new(&body_type_select)
                            .with_size(Size::Small)
                            .w(px(150.0)),
                    )
                    .child(div().flex_1())
                    .when(self.body_type == BodyType::Json, |this| {
                        this.child(
                            Button::new("format-json")
                                .with_size(Size::Small)
                                .ghost()
                                .label("Format")
                                .on_click(cx.listener(Self::handle_format_json)),
                        )
                    }),
            )
            .when(self.body_type == BodyType::Json, |this| {
                this.child(
                    gpui_component::input::Input::new(&body_input)
                        .h(px(300.0))
                        .flex_1()
                )
            })
            .when(self.body_type == BodyType::FormData, |this| {
                this.child(Self::render_form_data_fields(self, window, cx))
            })
    }
    
    fn render_form_data_fields(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .flex_1()
            .children(self.body_form_fields.iter().enumerate().map(|(index, field)| {
                let key_input = field.key_input.clone();
                let value_input = field.value_input.clone();
                let file_path = field.file_path.clone();
                let enabled = field.enabled;
                let value_type = field.value_type;
                
                let field_index = index;
                
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Checkbox::new(("form-enabled", index))
                            .checked(enabled)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if field_index < this.body_form_fields.len() {
                                    this.body_form_fields[field_index].enabled = !this.body_form_fields[field_index].enabled;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        gpui_component::input::Input::new(&key_input)
                            .w(px(160.0)),
                    )
                    .when(value_type == FormDataValueType::Text, |this| {
                        this.child(
                            gpui_component::input::Input::new(&value_input)
                                .flex_1(),
                        )
                    })
                    .when(value_type == FormDataValueType::File, |this| {
                        let display_name = file_path.as_ref().map(|p| {
                            std::path::Path::new(p)
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| p.clone())
                        }).unwrap_or_else(|| "Select file...".to_string());
                        
                        this.child(
                            div()
                                .flex_1()
                                .px_2()
                                .py_1()
                                .bg(cx.theme().background)
                                .border_1()
                                .border_color(cx.theme().input)
                                .rounded_sm()
                                .text_sm()
                                .cursor_pointer()
                                .text_color(if file_path.is_some() { cx.theme().foreground } else { cx.theme().muted_foreground })
                                .child(display_name)
                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                    this.handle_browse_file(field_index, window, cx);
                                })),
                        )
                    })
                    .child(
                        Button::new(("form-type-toggle", index))
                            .with_size(Size::Small)
                            .outline()
                            .label(if value_type == FormDataValueType::Text { "Text" } else { "File" })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if field_index < this.body_form_fields.len() {
                                    let current = this.body_form_fields[field_index].value_type;
                                    this.body_form_fields[field_index].value_type = match current {
                                        FormDataValueType::Text => FormDataValueType::File,
                                        FormDataValueType::File => FormDataValueType::Text,
                                    };
                                    this.body_form_fields[field_index].file_path = None;
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        Button::new(("form-delete", index))
                            .with_size(Size::Small)
                            .ghost()
                            .icon(IconName::Delete)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if field_index < this.body_form_fields.len() {
                                    this.body_form_fields.remove(field_index);
                                    cx.notify();
                                }
                            })),
                    )
            }))
            .child(
                Button::new("add-form-field")
                    .with_size(Size::Small)
                    .ghost()
                    .label("+ Add Field")
                    .on_click(cx.listener(Self::handle_add_form_field)),
            )
    }
    
    fn handle_add_form_field(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.body_form_fields.push(FormDataRow::new(window, cx));
        cx.notify();
    }
    
    fn handle_browse_file(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.body_form_fields.len() {
            return;
        }
        
        let future = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select a file".into()),
        });
        
        let field_index = index;
        let view = cx.entity().clone();
        let value_input = self.body_form_fields[index].value_input.clone();
        
        window.spawn(cx, async move |cx| {
            if let Ok(Ok(Some(paths))) = future.await {
                if let Some(path) = paths.first() {
                    let path_str = path.to_string_lossy().to_string();
                    let _ = view.update(cx, |this, cx| {
                        if field_index < this.body_form_fields.len() {
                            this.body_form_fields[field_index].file_path = Some(path_str.clone());
                            cx.notify();
                        }
                    });
                }
            }
        }).detach();
    }

    fn update_content_type_header(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let content_type = match self.body_type {
            BodyType::Json => "application/json",
            BodyType::FormData => "multipart/form-data",
        };
        
        let mut content_type_index: Option<usize> = None;
        let mut user_agent_index: Option<usize> = None;
        
        for (index, header) in self.headers.iter().enumerate() {
            let key = header.key_input.read(cx).text().to_string();
            let key_lower = key.to_lowercase();
            
            if key_lower.is_empty() {
                if let Some(ref dk) = header.default_key {
                    let dk_lower = dk.to_lowercase();
                    if dk_lower == "content-type" {
                        content_type_index = Some(index);
                    }
                    if dk_lower == "user-agent" {
                        user_agent_index = Some(index);
                    }
                }
            } else {
                if key_lower == "content-type" {
                    content_type_index = Some(index);
                }
                if key_lower == "user-agent" {
                    user_agent_index = Some(index);
                }
            }
        }
        
        if let Some(idx) = content_type_index {
            self.headers[idx].default_value = Some(content_type.to_string());
        } else {
            let mut new_header = KeyValueRow::new(window, cx);
            new_header.default_key = Some("Content-Type".to_string());
            new_header.default_value = Some(content_type.to_string());
            
            if let Some(ua_idx) = user_agent_index {
                self.headers.insert(ua_idx + 1, new_header);
            } else {
                self.headers.insert(0, new_header);
            }
        }
        
        if user_agent_index.is_none() {
            let user_agent_header = KeyValueRow::with_defaults("User-Agent", "OneHub/1.0", window, cx);
            if let Some(ct_idx) = content_type_index {
                self.headers.insert(ct_idx, user_agent_header);
            } else {
                self.headers.insert(0, user_agent_header);
            }
        }
    }

    fn handle_format_json(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.body_input.read(cx).text().to_string();
        if text.trim().is_empty() {
            return;
        }
        
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Ok(formatted) = serde_json::to_string_pretty(&json) {
                self.body_input.update(cx, |input, cx| {
                    input.set_value(formatted, window, cx);
                });
            }
        }
    }

    fn render_auth_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(div().child("Auth Type:"))
                    .child(div().child("None (No authentication)")),
            )
            .child(
                div()
                    .p_4()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .child("Auth configuration - Coming soon"),
            )
    }

    fn render_response_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let response = self.response_status.clone();
        let active_tab = self.active_response_tab;

        v_flex()
            .flex_1()
            .min_w(px(300.0))
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .px_4()
                    .py_2()
                    .gap_4()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        response.as_ref().map(|r| {
                            let status_color = if r.status >= 500 {
                                gpui::Hsla::red()
                            } else if r.status >= 400 {
                                gpui::Hsla { h: 0.08, s: 0.9, l: 0.5, a: 1.0 } // 橙色
                            } else if r.status >= 300 {
                                gpui::Hsla::blue()
                            } else if r.status >= 200 {
                                gpui::Hsla::green()
                            } else {
                                gpui::Hsla { h: 0.0, s: 0.0, l: 0.5, a: 1.0 } // 灰色
                            };
                            
                            h_flex().gap_2().items_center()
                                .child(
                                    gpui_component::badge::Badge::new()
                                        .color(status_color)
                                        .child(format!("{} {}", r.status, r.status_text))
                                )
                                .child(
                                    gpui_component::badge::Badge::new()
                                        .child(format!("{}ms", r.time_ms))
                                )
                                .child(
                                    gpui_component::badge::Badge::new()
                                        .child(format!("{} bytes", r.body.len()))
                                )
                        }).unwrap_or_else(|| div().child("").flex_1())
                    ),
            )
            .child(
                h_flex()
                    .px_4()
                    .py_1()
                    .gap_2()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .children([
                        ("resp-body", "Body", ResponsePanel::Body),
                        ("resp-headers", "Headers", ResponsePanel::Headers),
                        ("resp-cookies", "Cookies", ResponsePanel::Cookies),
                    ].map(|(id, label, panel)| {
                        let is_active = active_tab == panel;
                        if is_active {
                            Button::new(id)
                                .with_size(Size::Small)
                                .primary()
                                .label(label)
                                .on_click(cx.listener(move |this, event, window, cx| {
                                    this.active_response_tab = panel;
                                    cx.notify();
                                }))
                        } else {
                            Button::new(id)
                                .with_size(Size::Small)
                                .ghost()
                                .label(label)
                                .on_click(cx.listener(move |this, event, window, cx| {
                                    this.active_response_tab = panel;
                                    cx.notify();
                                }))
                        }
                    }))
            )
            .child(
                match (response.as_ref(), active_tab) {
                    (Some(r), ResponsePanel::Body) => {
                        let body_content = response.as_ref().map(|rr| rr.body.clone()).unwrap_or_default();
                        
                        if self.response_body_input.is_none() {
                            self.response_body_input = Some(cx.new(|cx| {
                                gpui_component::input::InputState::new(window, cx)
                                    .code_editor("json")
                                    .multi_line(true)
                            }));
                        }
                        
                        if let Some(ref input_state) = self.response_body_input {
                            input_state.update(cx, |state, cx| {
                                state.set_value(body_content, window, cx);
                            });
                            
                            gpui_component::input::Input::new(input_state)
                                .size_full()
                                .cleanable(false)
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        }
                    }
                            (Some(r), ResponsePanel::Headers) => {
                                v_flex()
                                    .p_4()
                                    .gap_1()
                                    .children(r.headers.iter().enumerate().map(|(idx, (k, v))| {
                                        let bg = if idx % 2 == 0 {
                                            cx.theme().muted
                                        } else {
                                            cx.theme().background
                                        };
                                        h_flex()
                                            .gap_2()
                                            .items_start()
                                            .text_sm()
                                            .font_family(cx.theme().mono_font_family.clone())
                                            .bg(bg)
                                            .px_2()
                                            .py_1()
                                            .rounded_sm()
                                            .child(
                                                div()
                                                    .w(px(180.0))
                                                    .flex_shrink_0()
                                                    .text_color(cx.theme().foreground)
                                                    .font_bold()
                                                    .child(Self::format_header_key(k))
                                            )
                                            .child(div().flex_1().text_color(cx.theme().foreground).child(v.clone()))
                                    }))
                                    .into_any_element()
                            }
                            (Some(r), ResponsePanel::Cookies) => {
                                v_flex()
                                    .p_4()
                                    .gap_1()
                                    .text_sm()
                                    .child(
                                        if r.cookies.is_empty() {
                                            div().text_color(cx.theme().muted_foreground).child("No cookies")
                                        } else {
                                            div().children(r.cookies.iter().map(|(k, v)| {
                                                div().child(format!("{}: {}", k, v))
                                            }))
                                        }
                                    )
                                    .into_any_element()
                            }
                            _ => div().into_any_element()
                        }
            )
    }
}

impl Focusable for ApiTabView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TabContent for ApiTabView {
    fn content_key(&self) -> &'static str {
        "Api"
    }

    fn title(&self, _cx: &App) -> SharedString {
        "API".into()
    }

    fn icon(&self, _cx: &App) -> Option<Icon> {
        Some(Icon::new(IconName::Globe))
    }

    fn dump(&self, _cx: &App) -> serde_json::Value {
        serde_json::Value::Null
    }

    fn restore(&mut self, _state: serde_json::Value, _cx: &mut gpui::Context<Self>) {}
}

impl gpui::EventEmitter<TabContentEvent> for ApiTabView {}

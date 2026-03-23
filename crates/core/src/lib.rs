use gpui::App;

rust_i18n::i18n!("locales", fallback = "zh-CN");

pub mod agent;
pub mod ai_chat;
pub mod cloud_api;
pub mod config;
pub mod connection_notifier;
pub mod gpui_tokio;
pub mod layout;
pub mod llm;
pub mod popup_window;
pub mod storage;
pub mod tab_container;
pub mod tab_persistence;
pub mod themes;
pub mod user_state;
pub mod utils;

pub use crate::cloud_api::CloudApiClient;
pub use crate::user_state::{GlobalUserState, TeamOption, UserInfo};

pub use crate::agent::{
    Agent, AgentContext, AgentDescriptor, AgentDispatcher, AgentEvent, AgentRegistry, AgentResult,
    SessionAffinity,
};
pub use crate::ai_chat::{
    AiChatColors, AiChatPanel, AiChatPanelEvent, ChatMessageUI, ChatMessageUIGeneric, ChatRole,
    CodeBlockAction, CodeBlockActionBuilder, CodeBlockActionCallback, CodeBlockActionRegistry,
    LanguageMatcher, MessageExtension, MessageVariant, NoExtension, ProviderItem,
};
pub use crate::ai_chat::{
    ChatEngine, ChatMessageRenderer, ChatStreamProcessor, CoreStreamEvent, StreamError,
};

pub fn init(cx: &mut App) {
    gpui_tokio::init(cx);
    themes::init(cx);
    storage::init(cx);
    llm::init(cx);
    agent::init(cx);
    connection_notifier::init(cx);
}

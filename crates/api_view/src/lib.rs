//! API 测试视图层

rust_i18n::i18n!("locales", fallback = "zh-CN");

pub mod models;
pub mod global_state;
pub mod api_tab;
pub mod history_storage;
pub mod collections_storage;

pub use api_tab::ApiTabView;
pub use global_state::GlobalApiState;
pub use history_storage::HistoryStorage;
pub use collections_storage::CollectionsStorage;
pub use models::*;

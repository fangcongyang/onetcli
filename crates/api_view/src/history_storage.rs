//! API History 存储管理

use crate::api_tab::HistoryItem;
use std::fs;
use std::path::PathBuf;

const HISTORY_FILE: &str = "history.json";
const MAX_HISTORY_ITEMS: usize = 100;

pub struct HistoryStorage;

impl HistoryStorage {
    /// 获取 history 文件路径
    fn get_history_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".config");
        path.push("one-hub");
        path.push("api");
        fs::create_dir_all(&path).ok();
        path.push(HISTORY_FILE);
        path
    }

    /// 加载历史记录
    pub fn load() -> Vec<HistoryItem> {
        let path = Self::get_history_path();
        if !path.exists() {
            return Vec::new();
        }

        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                log::warn!("Failed to parse history: {}", e);
                Vec::new()
            }),
            Err(e) => {
                log::warn!("Failed to read history file: {}", e);
                Vec::new()
            }
        }
    }

    /// 保存历史记录
    pub fn save(history: &[HistoryItem]) {
        let path = Self::get_history_path();

        // 只保存最新的 N 条记录
        let history_to_save: Vec<_> = history.iter().take(MAX_HISTORY_ITEMS).cloned().collect();

        match serde_json::to_string_pretty(&history_to_save) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json) {
                    log::warn!("Failed to write history: {}", e);
                }
            }
            Err(e) => {
                log::warn!("Failed to serialize history: {}", e);
            }
        }
    }

    /// 添加一条历史记录
    pub fn add_item(item: HistoryItem) {
        let mut history = Self::load();

        // 检查是否已存在相同的 URL 和方法组合，如果是则删除旧的
        history.retain(|h| !(h.url == item.url && h.method == item.method));

        // 添加到开头
        history.insert(0, item);

        // 限制数量
        history.truncate(MAX_HISTORY_ITEMS);

        // 保存
        Self::save(&history);
    }
}

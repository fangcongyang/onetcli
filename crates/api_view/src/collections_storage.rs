//! API Collections 存储管理

use crate::models::ApiCollection;
use std::fs;
use std::path::PathBuf;

const COLLECTIONS_FILE: &str = "collections.json";

pub struct CollectionsStorage;

impl CollectionsStorage {
    /// 获取 collections 文件路径
    fn get_collections_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".config");
        path.push("one-hub");
        path.push("api");
        fs::create_dir_all(&path).ok();
        path.push(COLLECTIONS_FILE);
        path
    }

    /// 加载所有 collections
    pub fn load() -> Vec<ApiCollection> {
        let path = Self::get_collections_path();
        if !path.exists() {
            return Vec::new();
        }

        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                tracing::warn!("Failed to parse collections: {}", e);
                Vec::new()
            }),
            Err(e) => {
                tracing::warn!("Failed to read collections file: {}", e);
                Vec::new()
            }
        }
    }

    /// 保存所有 collections
    pub fn save(collections: &[ApiCollection]) {
        let path = Self::get_collections_path();

        match serde_json::to_string_pretty(collections) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json) {
                    tracing::warn!("Failed to write collections: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize collections: {}", e);
            }
        }
    }

    /// 添加一个 collection
    pub fn add_collection(collection: ApiCollection) {
        let mut collections = Self::load();
        collections.push(collection);
        Self::save(&collections);
    }

    /// 更新一个 collection
    pub fn update_collection(collection: &ApiCollection) {
        let mut collections = Self::load();
        if let Some(existing) = collections.iter_mut().find(|c| c.id == collection.id) {
            *existing = collection.clone();
            Self::save(&collections);
        }
    }

    /// 删除一个 collection
    pub fn delete_collection(collection_id: uuid::Uuid) {
        let mut collections = Self::load();
        collections.retain(|c| c.id != collection_id);
        Self::save(&collections);
    }
}

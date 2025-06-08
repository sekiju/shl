pub mod error;

#[cfg(feature = "macro")]
extern crate shl_service_cache_macro;

#[cfg(feature = "macro")]
pub use shl_service_cache_macro::*;

use crate::error::Error;
use rustis::client::Client;
use rustis::commands::{GenericCommands, ScanOptions, StringCommands};
use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Clone)]
pub struct CacheService {
    client: Client,
    ttl: u64,
}

impl CacheService {
    pub fn new(client: Client, ttl: u64) -> Self {
        Self { client, ttl }
    }
}

impl CacheService {
    pub async fn set<T>(&self, key: &str, value: T) -> Result<(), Error>
    where
        T: Serialize,
    {
        let serialized = serde_json::to_string(&value)?;
        self.client.setex(key, self.ttl, serialized).await?;
        Ok(())
    }

    pub async fn get<T>(&self, key: &str) -> Option<T>
    where
        T: DeserializeOwned,
    {
        let value: Option<String> = self.client.get(key).await.ok()?;
        value.and_then(|cached| serde_json::from_str(&cached).ok())
    }

    pub async fn delete(&self, key: &str) -> Result<(), Error> {
        self.client.unlink(key).await?;
        Ok(())
    }

    pub async fn delete_pattern(&self, pattern: &str) -> Result<(), Error> {
        let mut cursor = 0_u64;

        loop {
            let (next, keys): (u64, Vec<String>) = self.client.scan(cursor, ScanOptions::default().match_pattern(pattern)).await?;

            if !keys.is_empty() {
                self.client.unlink(keys).await?;
            }

            if next == 0 {
                break;
            }

            cursor = next;
        }

        Ok(())
    }

    pub async fn delete_many(&self, keys: impl IntoIterator<Item = impl AsRef<str>>) -> Result<(), Error> {
        let keys: Vec<String> = keys.into_iter().map(|k| k.as_ref().to_string()).collect();
        self.client.unlink(keys).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustis::client::Client;
    use rustis::commands::{FlushingMode, ServerCommands};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        value: String,
    }
    
    async fn setup() -> CacheService {
        let client = Client::connect("redis://127.0.0.1/").await.unwrap();
        client.flushall(FlushingMode::Default).await.unwrap();
        CacheService::new(client, 60)
    }

    #[tokio::test]
    async fn test_set_and_get() {
        let cache = setup().await;
        let data = TestData { value: "test".into() };

        cache.set("key", &data).await.unwrap();
        let retrieved: Option<TestData> = cache.get("key").await;

        assert_eq!(retrieved, Some(data));
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let cache = setup().await;

        let retrieved: Option<TestData> = cache.get("nonexistent").await;

        assert_eq!(retrieved, None);
    }

    #[tokio::test]
    async fn test_delete() {
        let cache = setup().await;
        let data = TestData { value: "delete".into() };

        cache.set("to_delete", &data).await.unwrap();
        cache.delete("to_delete").await.unwrap();

        let retrieved: Option<TestData> = cache.get("to_delete").await;

        assert_eq!(retrieved, None);
    }

    #[tokio::test]
    async fn test_delete_pattern() {
        let cache = setup().await;

        cache.set("pattern_1", &"data1").await.unwrap();
        cache.set("pattern_2", &"data2").await.unwrap();

        cache.delete_pattern("pattern_*").await.unwrap();

        let res1: Option<String> = cache.get("pattern_1").await;
        let res2: Option<String> = cache.get("pattern_2").await;

        assert_eq!(res1, None);
        assert_eq!(res2, None);
    }

    #[tokio::test]
    async fn test_delete_many() {
        let cache = setup().await;

        cache.set("key1", &"data1").await.unwrap();
        cache.set("key2", &"data2").await.unwrap();

        cache.delete_many(vec!["key1", "key2"]).await.unwrap();

        let res1: Option<String> = cache.get("key1").await;
        let res2: Option<String> = cache.get("key2").await;

        assert_eq!(res1, None);
        assert_eq!(res2, None);
    }
}
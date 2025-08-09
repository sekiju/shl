use crate::{CacheClient, CacheService, Error};
use async_trait::async_trait;
use rustis::client::Client;
use rustis::commands::{GenericCommands, ScanOptions, StringCommands};
use std::sync::Arc;

pub type RedisCacheService = Arc<CacheService<Client>>;

#[async_trait]
impl CacheClient for Client {
    async fn get_raw(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        let value: Option<String> = self.get(key).await?;
        Ok(value.map(String::into_bytes))
    }

    async fn set_raw(&self, key: &str, ttl: u64, value: &[u8]) -> Result<(), Error> {
        self.setex(key, ttl, value).await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), Error> {
        self.unlink(key).await?;
        Ok(())
    }

    async fn delete_pattern(&self, pattern: &str) -> Result<(), Error> {
        let mut cursor = 0u64;
        loop {
            let (next, keys): (u64, Vec<String>) = self.scan(cursor, ScanOptions::default().match_pattern(pattern)).await?;
            if !keys.is_empty() {
                self.unlink(keys).await?;
            }
            if next == 0 {
                break;
            }
            cursor = next;
        }
        Ok(())
    }

    async fn delete_keys(&self, keys: impl IntoIterator<Item = impl AsRef<str> + Send> + Send) -> Result<(), Error> {
        let keys: Vec<String> = keys.into_iter().map(|k| k.as_ref().to_string()).collect();
        if !keys.is_empty() {
            self.unlink(keys).await?;
        }
        Ok(())
    }
}

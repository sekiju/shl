pub mod error;

#[cfg(feature = "macro")]
extern crate shl_service_cache_macro;

#[cfg(feature = "macro")]
pub use shl_service_cache_macro::*;

use crate::error::Error;
use rustis::client::Client;
use rustis::commands::{GenericCommands, StringCommands};
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
        let keys: Vec<String> = self.client.keys(pattern).await?;
        self.client.unlink(keys).await?;
        Ok(())
    }

    pub async fn delete_many(&self, keys: impl IntoIterator<Item = impl AsRef<str>>) -> Result<(), Error> {
        let keys: Vec<String> = keys.into_iter().map(|k| k.as_ref().to_string()).collect();
        self.client.unlink(keys).await?;
        Ok(())
    }
}

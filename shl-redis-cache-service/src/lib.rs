use crate::error::Error;
use async_trait::async_trait;
use serde::Serialize;
use serde::de::DeserializeOwned;

#[cfg(feature = "macro")]
pub use shl_redis_cache_macro::*;

mod error;
#[cfg(feature = "rustis")]
pub mod rustis;

#[async_trait]
pub trait CacheClient: Send + Sync {
    async fn get_raw(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;
    async fn set_raw(&self, key: &str, ttl: u64, value: &[u8]) -> Result<(), Error>;
    async fn delete(&self, key: &str) -> Result<(), Error>;
    async fn delete_pattern(&self, pattern: &str) -> Result<(), Error>;
    async fn delete_keys(&self, keys: impl IntoIterator<Item = impl AsRef<str> + Send> + Send) -> Result<(), Error>;
}

#[derive(Clone)]
pub struct CacheService<C: CacheClient> {
    client: C,
    ttl: u64,
}

impl<C: CacheClient> CacheService<C> {
    pub fn new(client: C, ttl: u64) -> Self {
        Self { client, ttl }
    }

    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), Error> {
        let serialized = serde_json::to_vec(value)?;
        self.client.set_raw(key, self.ttl, &serialized).await
    }

    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, Error> {
        match self.client.get_raw(key).await? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    pub async fn delete(&self, key: &str) -> Result<(), Error> {
        self.client.delete(key).await
    }

    pub async fn delete_pattern(&self, pattern: &str) -> Result<(), Error> {
        self.client.delete_pattern(pattern).await
    }

    pub async fn delete_keys(&self, keys: impl IntoIterator<Item = impl AsRef<str> + Send> + Send) -> Result<(), Error> {
        self.client.delete_keys(keys).await
    }
}

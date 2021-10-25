//!
//! Strongbox clients
//!
use crate::{Client, Error};
use std::collections::HashMap;

use crate::Result;

/// Certificate API
pub mod tls;

const SBOX_VAULTS: &str = "v1/config/strongbox/vault";

/// A Strobox vault can contain one or more `KVMap` key value stores.
pub struct Vault {
    client: Client,
    vault_url: String,
}

impl Vault {
    /// List all secret stores
    pub async fn list(client: &Client) -> Result<Vec<String>> {
        let resp: serde_json::Value = client.get_json(SBOX_VAULTS, Some(&[("keys", "")])).await?;

        resp.as_array()
            .ok_or_else(|| crate::Error::API("Expected an array".into()))?
            .iter()
            .inspect(|s| tracing::debug!("{:#?}", s))
            .map(|s| {
                s.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| crate::Error::API("Expected a name".into()))
            })
            .collect()
    }

    pub(crate) async fn open(client: &Client, vault: &str) -> Result<Self> {
        let vault_url = format!("{}/{}", SBOX_VAULTS, vault);
        tracing::debug!("Opening vault at path: {}", vault_url);
        // Try to get the sbox vault
        let _: serde_json::Value = client.get_json(&vault_url, None).await?;
        Ok(Self {
            client: client.clone(),
            vault_url,
        })
    }

    /// Open a KV map
    pub async fn open_kv_map(&self, name: &str) -> Result<KVMap> {
        let map_url = format!("{}/kv-maps/{}", self.vault_url, name);

        let json: serde_json::Value = self.client.get_json(&map_url, None).await?;

        let kv = json
            .as_object()
            .ok_or_else(|| Error::general("expected a JSON object in kv-map"))?;

        let mut cache = HashMap::new();
        if let Some(data) = kv.get("data").map(|d| d.as_object()).flatten() {
            for (k, v) in data.into_iter() {
                cache.insert(
                    k.clone(),
                    v.as_str()
                        .ok_or_else(|| Error::general("Expected secret value to be a string"))?
                        .to_string(),
                );
            }
        }

        tracing::debug!("Successfully loaded {}", name);

        Ok(KVMap { cache })
    }
}

/// Strongbox key value map
#[derive(Clone)]
pub struct KVMap {
    cache: HashMap<String, String>,
}

impl KVMap {
    /// Try to get a value
    pub fn get(&self, key: &str) -> Option<&String> {
        self.cache.get(key)
    }
}

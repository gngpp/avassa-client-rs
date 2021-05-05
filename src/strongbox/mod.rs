//!
//! Strongbox clients
//!
// use crate::Client;
// use log::debug;
// use serde_json::json;
// use std::collections::HashMap;

// use crate::Result;

/// Certificate API
pub mod tls;

/*
const SBOX_SECRETS: &str = "v1/config/strongbox/secrets";

/// A Strobox secret store can contain one or more `KVMap` key value stores.
pub struct SecretStore {
    client: Client,
    store_url: String,
}

impl SecretStore {
    /// List all secret stores
    pub async fn list(client: &Client) -> Result<Vec<String>> {
        let resp: serde_json::Value = client
            .get_json(&format!("{}/live", SBOX_SECRETS), Some(&[("keys", "")]))
            .await?;

        resp.as_array()
            .ok_or(crate::Error::API("Expected an array".into()))?
            .into_iter()
            .map(|s| {
                debug!("{:?}", s);
                s.as_str()
                    .map(str::to_string)
                    .ok_or(crate::Error::API("Expected a name".into()))
            })
            .collect()
    }

    async fn new(client: Client, name: &str, distribute: bool) -> Result<Self> {
        let create = json!( {
            "name": name,
            "distribute": distribute,
        });

        client
            .post_json(&format!("{}/live", SBOX_SECRETS), &create)
            .await?;

        let store_url = format!("{}/live/{}", SBOX_SECRETS, name);

        Ok(Self { client, store_url })
    }

    /// Creates or openes a new secret store that is distributed to all downstream
    /// data centers
    pub async fn new_distributed(client: Client, name: &str) -> Result<Self> {
        Self::new(client, name, true).await
    }

    /// Creates or openes a new secret store that is local to this datacenter
    pub async fn new_local(client: Client, name: &str) -> Result<Self> {
        Self::new(client, name, false).await
    }

    /// Open or create a key value map
    pub async fn kv_map(&self, name: &str) -> Result<KVMap> {
        let create = json!( {
            "name": name,
            "data": {}
        });

        let client = self.client.clone();
        client
            .post_json(&format!("{}/kv_maps", self.store_url), &create)
            .await?;

        let _map_url = format!("{}/kv_maps/{}", self.store_url, name);
        todo!("get content from sbox");
        // Ok(KVMap {
        //     client,
        //     map_url,
        //     cache: HashMap::new(),
        //     dirty: false,
        // })
    }
}

/// Strongbox key value map
pub struct KVMap {
    _client: Client,
    _map_url: String,
    cache: HashMap<String, String>,
    _dirty: bool,
}

impl KVMap {
    /// Insert key and value
    pub fn insert(&mut self, key: String, value: String) -> Option<String> {
        self.cache.insert(key, value)
    }

    /// Try to get a value
    pub fn get(&self, key: &str) -> Option<&String> {
        self.cache.get(key)
    }

    /// Try to remove an entry
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.cache.remove(key)
    }
}
*/

//!
//! Collection of types returned from the APIs.
//!
//! NOTE: The types here are not exhaustive to the APIs.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Volga topic state
#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Topic {
    /// Topic name
    pub name: String,
    /// Tenant owner
    pub tenant: String,
    /// Message format
    pub format: crate::volga::Format,
    /// Sequence number
    pub seqno: u64,
    /// Chunk number
    pub chunkno: u64,
    /// Total number of chunks
    pub number_of_chunks: u64,
    /// Topic creation time
    pub creation_time: chrono::DateTime<chrono::Local>,
    /// Hosts topic is replicated to
    pub assigned_hosts: Vec<String>,
    /// Replication hosts leader
    pub leader_host: String,
    /// Worker hosts
    pub worker_hosts: Vec<String>,
    /// Replicatation factor
    pub requested_replication_factor: usize,
    /// Replicatation factor
    pub current_replication_factor: usize,
    /// Persistence
    pub persistence: crate::volga::Persistence,
    /// Topic size
    // #[serde(rename = "size-megabyte")]
    // #[serde(deserialize_with = "parse_size")]
    // pub size: bytesize::ByteSize,
    /// Oldest entry timestamp
    pub oldest_entry: chrono::DateTime<chrono::Local>,
    /// Number of dropped chunks
    pub dropped_chunks: usize,
    /// Topic labels
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
struct Consumer {
    #[serde(rename = "consumer-name")]
    name: String,
    more_n: usize,
    last_ack: usize,
    buffered: usize,
    mode: crate::volga::consumer::Mode,
    consuming_host: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
struct Producer {
    #[serde(rename = "producer-name")]
    name: String,
    producing_host: String,
}

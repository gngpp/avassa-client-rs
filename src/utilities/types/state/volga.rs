//!
//! Collection of types returned from the APIs.
//!
//! NOTE: The types here are not exhaustive to the APIs.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Volga topic state
#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Topic {
    /// Topic name
    pub name: String,
    /// Tenant owner
    pub tenant: String,
    /// Message format
    pub format: crate::volga::Format,
    /// Sequence number
    #[serde(default)]
    pub seqno: u64,
    /// Chunk number
    #[serde(default)]
    pub chunkno: u64,
    /// Total number of chunks
    pub number_of_chunks: u64,
    /// Topic creation time
    pub creation_time: Option<chrono::DateTime<chrono::Local>>,
    /// Hosts topic is replicated to
    pub assigned_hosts: Vec<String>,
    /// Replication hosts leader
    pub leader_host: String,
    /// Worker hosts
    #[serde(default)]
    pub worker_hosts: Vec<String>,
    /// Replicatation factor
    pub requested_replication_factor: usize,
    /// Replicatation factor
    pub current_replication_factor: usize,
    /// Persistence
    pub persistence: crate::volga::Persistence,
    // /// Topic size
    // pub size: Option<String>,
    /// Oldest entry timestamp
    pub oldest_entry: Option<chrono::DateTime<chrono::Local>>,
    /// Number of dropped chunks
    #[serde(default)]
    pub dropped_chunks: usize,
    /// Topic labels
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
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

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
struct Producer {
    #[serde(rename = "producer-name")]
    name: String,
    producing_host: String,
}

// #[cfg(test)]
// mod test {
//     #[test]
//     fn parse_size() {
//         let sizes = ["976.56 KiB", "95.37 MiB"];
//         let sizes_bytes = [999997, 1000002593];
//         for (size, size_bytes) in sizes.iter().zip(sizes_bytes.iter()) {
//             let s: human_size::Size = size.parse().unwrap();
//             assert_eq!(s.to_bytes(), *size_bytes);
//         }
//     }
// }

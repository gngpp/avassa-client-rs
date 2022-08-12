//!
//! Cluster Host state
//!

/// Returned from /system/cluster/hosts
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Host {
    pub cluster_hostname: String,
    pub oper_status: OperStatus,
}

#[derive(Debug, serde::Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum OperStatus {
    Up,
    Down,
    DisasterMode,
}

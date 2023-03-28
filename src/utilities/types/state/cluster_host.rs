//!
//! Cluster Host state
//!

/// Returned from /system/cluster/hosts
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct Host {
    pub cluster_hostname: String,
    pub hostname: String,
    pub oper_status: OperStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, serde::Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum OperStatus {
    Up,
    Down,
    DisasterMode,
}

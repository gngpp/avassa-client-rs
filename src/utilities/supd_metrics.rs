/// Application/container metrics
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MetricEntry {
    /// Metric timestamp
    pub time: chrono::DateTime<chrono::Utc>,
    /// Site name
    pub site: String,
    /// Cluster host
    pub cluster_hostname: String,
    /// Site host
    pub hostname: String,

    /// CPU metrics
    pub cpu: CPU,

    /// Memory metrics
    pub memory: Memory,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CPU {
    pub nanoseconds: u64,
    pub cpus: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Memory {
    pub used: u64,
    pub total: u64,
    pub percentage_used: f64,
}

/// Host Metrics
#[allow(missing_docs)]

/// Memory
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Memory {
    pub total: u64,
    pub free: u64,
    pub available: u64,
}

/// Load average
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LoadAvg {
    /// 1 minute average
    pub avg1: f64,
    /// 5 minutes average
    pub avg5: f64,
    /// 15 minutes average
    pub avg15: f64,
    /// Number of currently runnable kernel scheduling entities (processes,
    pub running: u64,
    /// Number of kernel scheduling entities (processes, threads) that currently exist on the system.";
    pub total: u64,
}

/// Disk
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Disk {
    pub filesystem: String,
    #[serde(rename = "type")]
    pub fs_type: String,
    pub size: u64,
    pub used: u64,
    pub free: u64,
    pub percentage_used: f32,
    pub mount: String,
}

/// Application/container metrics
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MetricData {
    pub site: String,
    pub hostname: String,
    pub memory: Memory,
    pub loadavg: LoadAvg,
    pub disk: Vec<Disk>,
}

/// Matrics
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Metrics {
    pub occurred_at: chrono::DateTime<chrono::FixedOffset>,
    pub tenant: String,
    pub data: MetricData,
}

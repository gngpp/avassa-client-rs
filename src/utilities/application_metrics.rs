//!
//! Metrics
//!

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Memory {
    pub used: u64,
    pub total: u64,
    pub percentage_used: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CPU {
    pub nanoseconds: u64,
    pub cpus: f64,
    pub shares: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContainerLayer {
    pub size: u64,
    pub used: u64,
    pub free: u64,
    pub percentage_used: f64,
}

/// Container metrics
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContainerMetric {
    /// Service instance name
    pub service_instance: String,
    /// Container name
    pub container: String,

    /// Memory consumed
    pub memory: Memory,
    /// CPU consumed
    pub cpu: CPU,

    /// Container layer
    pub container_layer: ContainerLayer,
}

/// Network metrics
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GatewayNetwork {
    /// Packets sent
    pub tx_packets: Option<u64>,
    /// Bytes sent
    pub tx_bytes: Option<u64>,
    /// Packets received
    pub rx_packets: Option<u64>,
    /// Bytes received
    pub rx_bytes: Option<u64>,
}

/// Appliction metrics
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ApplicationMetric {
    /// Network metrics
    pub gateway_network: Option<GatewayNetwork>,
}

/// Application/container metrics
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MetricEntry {
    /// Metric timestamp
    pub time: chrono::DateTime<chrono::Utc>,
    /// Site host
    pub host: String,
    /// Application name
    pub application: String,
    /// Container metrics
    pub per_container: Option<ContainerMetric>,
    /// Application metrics
    pub per_application: Option<ApplicationMetric>,
}

/// Matrics
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Metrics {
    /// Tenant name
    pub tenant: String,
    /// Site name
    pub site: String,
    /// Metric entires
    pub entries: Vec<MetricEntry>,
}

#[cfg(test)]
mod tests {}

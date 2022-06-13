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
    pub gateway_network: GatewayNetwork,
}

/// Application/container metrics
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MetricEntry {
    /// Metric timestamp
    pub time: chrono::DateTime<chrono::FixedOffset>,
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
    /// Metric entires
    pub entries: Vec<MetricEntry>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn network() {
        let json = r#"{
      "time": "2021-09-22T11:44:03.418Z",
      "host": "jkpg-001",
      "application": "popcorn-controller",
      "per-application": {
        "gateway-network": {
          "tx-packets": 1,
          "tx-bytes": 2,
          "rx-packets": 3,
          "rx-bytes": 4
        }
      }
    }"#;
        let entry: super::MetricEntry = serde_json::from_str(json).unwrap();
        assert!(entry.per_application.is_some());
        assert_eq!(
            entry
                .per_application
                .as_ref()
                .unwrap()
                .gateway_network
                .tx_packets,
            Some(1)
        );
        assert_eq!(
            entry
                .per_application
                .as_ref()
                .unwrap()
                .gateway_network
                .tx_bytes,
            Some(2)
        );
        assert_eq!(
            entry
                .per_application
                .as_ref()
                .unwrap()
                .gateway_network
                .rx_packets,
            Some(3)
        );
        assert_eq!(
            entry
                .per_application
                .as_ref()
                .unwrap()
                .gateway_network
                .rx_bytes,
            Some(4)
        );
    }

    // Removed untils metrics stabilize
    //
    // #[test]
    // fn per_application() {
    //     let json = r#" {
    //   "time": "2021-09-22T11:45:55.134Z",
    //   "host": "sthlm-002",
    //   "application": "popcorn-controller",
    //   "per-container": {
    //     "service-instance": "popcorn-controller-service-1",
    //     "container": "kettle-popper-manager",
    //     "memory-bytes": 2428928,
    //     "cpu-nanoseconds": 89024322
    //   }
    // }"#;
    //     let entry: super::MetricEntry = serde_json::from_str(json).unwrap();

    //     assert!(entry.per_container.is_some());
    //     assert_eq!(
    //         entry.per_container.as_ref().unwrap().service_instance,
    //         "popcorn-controller-service-1"
    //     );
    //     assert_eq!(
    //         entry.per_container.as_ref().unwrap().container,
    //         "kettle-popper-manager"
    //     );
    //     assert_eq!(
    //         entry.per_container.as_ref().unwrap().memory_bytes,
    //         Some(2428928)
    //     );
    //     assert_eq!(
    //         entry.per_container.as_ref().unwrap().cpu_nanoseconds,
    //         Some(89024322)
    //     );
    // }
}

//!
//! Application state
//!

#[allow(missing_docs)]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Application {
    pub name: String,
    pub config_modified_time: chrono::DateTime<chrono::Utc>,
    pub oper_status: OperStatus,
    pub service_instances: Vec<ServiceInstance>,
}

/// Service instance state
#[allow(missing_docs)]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ServiceInstance {
    pub name: String,
    pub oper_status: OperStatus,
    pub host: String,
    // application-network:
    //   ips:
    //     - 172.19.0.3/16
    // gateway-network:
    //   ips:
    //     - 172.18.255.4/24
    // ingress:
    //   ips: []
    // ephemeral-volumes:
    //   - name: cfg-storage
    //     size: 3MiB
    //     host-volume: volume1
    // init-containers:
    //   - name: setup
    //     oper-status: completed
    pub containers: Vec<Container>,

    pub ingress: Ingress,
}

/// Ingress information
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub struct Ingress {
    pub ips: Vec<std::net::IpAddr>,
}

/// State of the application
#[allow(missing_docs)]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperStatus {
    Running,
    AdminDown,
    Upgrading,
    Starting,
    Error,
    Failed,
}

/// Container state
#[allow(missing_docs)]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Container {
    pub name: String,
    pub id: String,
    pub oper_status: OperStatus,
    pub start_time: chrono::DateTime<chrono::Local>,
    pub current_restarts: u64,
    pub total_restarts: u64,
    //     probes:
    //       startup:
    //         status: success
    //       readiness:
    //         status: success
    //       liveness:
    //         status: success
}

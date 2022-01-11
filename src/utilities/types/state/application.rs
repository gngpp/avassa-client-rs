//!
//! Application state
//!

/// Service instance state
#[allow(missing_docs)]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ServiceInstance {
    pub name: String,
    pub oper_status: Operstatus,
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
}

/// State of the application
#[allow(missing_docs)]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Operstatus {
    Running,
    AdminDown,
    Upgrading,
    Starting,
    Error,
}

/// Container state
#[allow(missing_docs)]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Container {
    pub name: String,
    pub id: String,
    pub oper_status: Operstatus,
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

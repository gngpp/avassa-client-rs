#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AuditEntry {
    pub client_ip: std::net::IpAddr,
    pub host: String,
    pub method: String,
    pub path: String,
    pub request_parameters: Option<serde_json::Value>,
    pub request_time_ms: Option<usize>,
    pub site: String,
    pub status: Option<usize>,
    pub status_info: Option<String>,
    pub tenant: String,
    pub user: String,
    pub user_agent: Option<String>,
    pub x_forwarded_for: Option<std::net::IpAddr>,
    pub x_real_ip: Option<std::net::IpAddr>,
}

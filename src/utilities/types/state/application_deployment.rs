//!
//! Application Deployment state
//!

#[allow(missing_docs)]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ApplicationDeployment {
    pub name: String,
    pub application: String,
    pub application_version: String,
    pub status: Status,
}

#[allow(missing_docs)]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Status {
    pub oper_status: String,
    #[serde(default)]
    pub application_versions: Vec<ApplicationVersion>,
}

#[allow(missing_docs)]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ApplicationVersion {
    pub version: String,
    pub deployed_to: Vec<DeployedTo>,
}

#[allow(missing_docs)]
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DeployedTo {
    pub site: String,
    pub time: chrono::DateTime<chrono::Utc>,
}

//!
//! Optional utility functions
//!

pub mod application_metrics;
pub mod audit_trail;
pub mod host_metrics;
pub mod supd_metrics;
pub mod types;
pub mod alerts;

/// State utility functions
pub mod state {
    use super::types;
    use crate::Client;

    /// Returns a list of DCs a tenant can access
    pub async fn assigned_sites(client: &Client) -> crate::Result<Vec<types::state::site::Site>> {
        let sites = client.get_json("/v1/state/assigned-sites", None).await?;
        Ok(sites)
    }

    /// Return the site name
    pub async fn site_name(client: &Client) -> crate::Result<String> {
        client
            .get_json::<serde_json::Value>("/v1/state/system/cluster", None)
            .await?
            .as_object()
            .ok_or_else(|| crate::Error::general("Failed to get cluster information"))?
            .get("site-name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| crate::Error::general("Failed to get site name"))
            .map(Into::into)
    }

    /// Return an applications service instance state
    pub async fn service_instance(
        client: &Client,
        application: &str,
        service_instance: &str,
    ) -> crate::Result<types::state::application::ServiceInstance> {
        let si = client
            .get_json(
                &format!(
                    "/v1/state/applications/{}/service-instances/{}",
                    application, service_instance
                ),
                None,
            )
            .await?;

        Ok(si)
    }

    /// Return an applications service instances state
    pub async fn service_instances(
        client: &Client,
        application: &str,
    ) -> crate::Result<Vec<types::state::application::ServiceInstance>> {
        let si = client
            .get_json(
                &format!("/v1/state/applications/{}/service-instances", application),
                None,
            )
            .await?;

        Ok(si)
    }

    pub async fn applications(
        client: &Client,
    ) -> crate::Result<Vec<types::state::application::Application>> {
        let app = client
            .get_json(&format!("/v1/state/applications"), None)
            .await?;

        Ok(app)
    }

    pub async fn application(
        client: &Client,
        application: &str,
    ) -> crate::Result<types::state::application::Application> {
        let app = client
            .get_json(&format!("/v1/state/applications/{}", application), None)
            .await?;

        Ok(app)
    }

    pub async fn cluster_hosts(
        client: &Client,
    ) -> crate::Result<Vec<types::state::cluster_host::Host>> {
        let c = client
            .get_json("/v1/state/system/cluster/hosts", None)
            .await?;
        Ok(c)
    }
}

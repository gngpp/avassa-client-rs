//!
//! Optional utility functions
//!

pub mod application_metrics;
pub mod types;

/// State utility functions
pub mod state {
    use super::types;
    use crate::Client;

    /// Returns a list of DCs a tenant can access
    pub async fn assigned_sites(client: &Client) -> crate::Result<Vec<types::state::Site>> {
        let sites = client.get_json("/v1/state/assigned-sites", None).await?;
        Ok(sites)
    }

    /// Return the site name
    pub async fn site_name(client: &Client) -> crate::Result<String> {
        client
            .get_json::<serde_json::Value>("/v1/state/cluster", None)
            .await?
            .as_object()
            .ok_or_else(|| crate::Error::general("Failed to get cluster information"))?
            .get("cluster-id")
            .map(serde_json::Value::as_str)
            .flatten()
            .ok_or_else(|| crate::Error::general("Failed to get cluster_id"))
            .map(Into::into)
    }
}

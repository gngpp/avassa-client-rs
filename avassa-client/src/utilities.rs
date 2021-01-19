//!
//! Collection of utility functions
//!

/// State utility functions
pub mod state {
    use crate::Client;
    /// Returns a list of DCs a tenant can access
    pub async fn datacenters(client: &Client) -> crate::Result<Vec<crate::types::state::DC>> {
        let dc = client.get_json("/v1/state/datacenters", None).await?;
        Ok(dc)
    }
}

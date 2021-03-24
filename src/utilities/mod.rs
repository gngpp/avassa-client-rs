//!
//! Collection of utility functions
//!

pub mod types;

/// State utility functions
pub mod state {
    use crate::utilities::types;
    use crate::Client;
    /// Returns a list of DCs a tenant can access
    pub async fn assigned_datacenters(client: &Client) -> crate::Result<Vec<types::state::DC>> {
        let dc = client
            .get_json("/v1/state/assigned_datacenters", None)
            .await?;
        Ok(dc)
    }
}

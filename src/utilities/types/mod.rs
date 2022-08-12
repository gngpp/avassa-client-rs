//!
//! Collection of types returned from the APIs.
//!
//! NOTE: The types here are not exhaustive to the APIs.
use serde::{Deserialize, Serialize};

/// Type of datacenter
#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
pub enum SiteType {
    /// Edge Enforcer
    #[serde(rename = "edge")]
    Edge,
    /// Control Tower
    #[serde(rename = "top")]
    Top,
}

/// Optional location of a DC
#[derive(Debug, Deserialize, Serialize)]
pub struct SiteLocation {
    /// Latitude
    pub latitude: f64,
    /// Longitude
    pub longitude: f64,
    /// Location Description
    pub description: Option<String>,
}

/// Config types
pub mod config {}

/// State types
pub mod state {
    pub mod application;
    pub mod application_deployment;
    pub mod cluster_host;
    pub mod site;
    pub mod volga;
}

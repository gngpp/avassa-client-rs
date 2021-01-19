//!
//! Collection of types returned from the APIs.
//!
//! NOTE: The types here are not exhaustive to the APIs.
use serde::{Deserialize, Serialize};

/// Type of datacenter
#[derive(Debug, Deserialize, Serialize)]
pub enum DCType {
    /// Edge Enforcer
    #[serde(rename = "edge")]
    Edge,
    /// Control Tower
    #[serde(rename = "top")]
    Top,
}

/// Optional location of a DC
#[derive(Debug, Deserialize, Serialize)]
pub struct DCLocation {
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
    use serde::Deserialize;

    /// Returned from /datacenters
    #[derive(Debug, Deserialize)]
    pub struct DC {
        /// DC Name
        pub name: String,
        /// DC Type
        #[serde(rename = "type")]
        pub dc_type: crate::types::DCType,
        /// Optional DC Location
        pub location: Option<crate::types::DCLocation>,
    }
}

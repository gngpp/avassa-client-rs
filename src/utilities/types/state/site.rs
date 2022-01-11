//!
//! Site state
//!

/// Returned from /system/sites
#[derive(Debug, serde::Deserialize)]
pub struct Site {
    /// DC Name
    pub name: String,
    /// DC Type
    #[serde(rename = "type")]
    pub site_type: crate::utilities::types::SiteType,
    /// Optional DC Location
    pub location: Option<crate::utilities::types::SiteLocation>,
    /// Site labels
    pub labels: std::collections::HashMap<String, String>,
}

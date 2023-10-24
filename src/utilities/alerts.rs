//!
//! Alert management
//!

#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
/// Alert severity
pub enum Severity {
    Warning,
    Minor,
    Major,
    Critical,
}

/// Generate a custom alert
/// # Arguments
/// * `client` - an Avassa Client
/// * `id` - alert id
/// * `name` - alert name
/// * `description` - alert description
/// * `severity` - alert severity
pub async fn alert(
    client: &crate::Client,
    id: &str,
    name: &str,
    description: &str,
    severity: Severity,
) -> crate::Result<()> {
    let alert = serde_json::json!({
        "id": id,
        "name": name,
        "description": description,
        "severity": severity,
    });

    let _ = client.post_json("/v1/alert", &alert).await?;
    Ok(())
}

/// Clear a custom alert
/// # Arguments
/// * `client` - an Avassa Client
/// * `id` - alert id
/// * `name` - alert name
pub async fn clear_alert(client: &crate::Client, id: &str, name: &str) -> crate::Result<()> {
    let clear = serde_json::json!({
        "id": id,
        "name": name,
    });

    let _ = client.post_json("/v1/clear", &clear).await?;
    Ok(())
}

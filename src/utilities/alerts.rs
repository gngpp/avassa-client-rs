#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Severity {
    Warning,
    Minor,
    Major,
    Critical,
}

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

pub async fn clear_alert(client: &crate::Client, id: &str, name: &str) -> crate::Result<()> {
    let clear = serde_json::json!({
        "id": id,
        "name": name,
    });

    let _ = client.post_json("/v1/clear", &clear).await?;
    Ok(())
}

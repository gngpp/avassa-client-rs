#[derive(Clone, Copy)]
pub enum TokenType {
    User,
    Tenant,
    Seal,
}

/// Try to login using a supctl path
pub async fn login<P: AsRef<std::path::Path>>(
    supctl_path: P,
    token_type: TokenType,
) -> crate::Result<crate::Client> {
    let cfg_path = supctl_path.as_ref().join("cfg");
    let data = tokio::fs::read(cfg_path).await?;

    let cfg: Cfg = serde_json::from_slice(&data)?;

    let url = format!("https://{}:{}", cfg.host, cfg.port);

    let token_path = match token_type {
        TokenType::User => supctl_path.as_ref().join("user"),
        TokenType::Tenant => supctl_path.as_ref().join("tenant"),
        TokenType::Seal => supctl_path.as_ref().join("seal"),
    };

    let data = tokio::fs::read(token_path).await?;

    let token: Token = serde_json::from_slice(&data)?;

    let client = crate::Client::builder()
        .danger_accept_invalid_certs()
        .danger_accept_invalid_hostnames()
        .token_login(&url, &token.token)?;
    Ok(client)
}

#[derive(serde::Deserialize)]
struct Cfg {
    host: String,
    port: String,
}

#[derive(serde::Deserialize)]
struct Token {
    token: String,
}

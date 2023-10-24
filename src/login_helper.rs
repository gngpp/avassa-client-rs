//!
//! Optional module for logging in

/// Login from environment variables
/// * `SUPD` - typically set to `https://api:4646`
/// * `APPROLE_SECRET_ID` - set by the application specification
///
/// If no `APPROLE_SECRET_ID` is set, fallback to testing with `SUPD_USER` and `SUPD_PASSWORD`.
///
/// **NOTE** Username/password authentication should not be used in production but only for local testing.
///
pub async fn login() -> crate::Result<crate::Client> {
    let supd =
        std::env::var("SUPD").map_err(|_| crate::Error::LoginFailureMissingEnv("SUPD".into()))?;
    tracing::info!("Connecting to api at address {}", supd);

    let builder = crate::ClientBuilder::new();

    let ca_cert = std::env::var("SUPD_CA").map(|s| s.bytes().collect::<Vec<u8>>());

    let builder = if let Ok(ca) = ca_cert {
        builder.add_root_certificate(&ca)?
    } else {
        builder.danger_disable_cert_verification()
    };

    tracing::info!("Trying application login");
    if let Ok(client) = builder.application_login(&supd, None).await {
        return Ok(client);
    }

    tracing::info!("Trying username/password login");
    let username = std::env::var("SUPD_USER")
        .map_err(|_| crate::Error::LoginFailureMissingEnv("SUPD_USER".to_string()))?;
    let password = std::env::var("SUPD_PASSWORD")
        .map_err(|_| crate::Error::LoginFailureMissingEnv("SUPD_PASSWORD".to_string()))?;
    builder.login(&supd, &username, &password).await
}

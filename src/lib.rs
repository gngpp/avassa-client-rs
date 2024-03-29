//!
//! Library for interacting with an Avassa system.
//!
//! ## Avassa Client
//! The first interaction is to login into the system
//! ```no_run
//! #[tokio::main]
//! async fn main() -> Result<(), avassa_client::Error> {
//!     use avassa_client::Client;
//!
//!     // API CA certificate loaded
//!     let ca_cert = Vec::new();
//!
//!     // Use login using platform provided application token
//!     let approle_id = "secret approle id";
//!     let client = Client::builder()
//!         .add_root_certificate(&ca_cert)?
//!         .application_login("https://api.customer.net", Some(approle_id)).await?;
//!
//!     // Username and password authentication, good during the development phase
//!     let client = Client::builder()
//!         .add_root_certificate(&ca_cert)?
//!         .login("https://1.2.3.4", "joe", "secret").await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Volga
//! ### Create a Volga producer and consumer
//! ```no_run
//! #[tokio::main]
//! async fn main() -> Result<(), avassa_client::Error> {
//!     use avassa_client::Client;
//!
//!     // API CA certificate loaded
//!     let ca_cert = Vec::new();
//!
//!     // Use login using platform provided application token
//!     let approle_id = "secret approle id";
//!     let client = Client::builder()
//!         .add_root_certificate(&ca_cert)?
//!         .application_login("https://api.customer.net", Some(approle_id)).await?;
//!
//!     // Clone to move into async closure
//!     let producer_client = client.clone();
//!
//!     tokio::spawn(async move {
//!         let mut producer = producer_client.volga_open_producer(
//!             "test-producer",
//!             "my-topic",
//!             avassa_client::volga::OnNoExists::Create(Default::default())
//!             )
//!             .await?;
//!
//!         producer.produce(&serde_json::json!({
//!            "msg": "The Message",
//!         })).await?;
//!         Ok::<_, avassa_client::Error>(())
//!     });
//!
//!     let mut consumer = client.volga_open_consumer(
//!         "test-consumer",
//!         "my-topic",
//!         Default::default())
//!         .await?;
//!
//!     let message = consumer.consume::<String>().await?;
//!
//!     assert_eq!(message.payload, "test message");
//!     Ok(())
//! }
//! ```

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::cargo)]
#![allow(clippy::missing_errors_doc)]
use bytes::Bytes;
use serde::Deserialize;

use serde_json::json;

pub mod strongbox;
pub mod volga;

#[cfg(feature = "utilities")]
pub mod utilities;

#[cfg(feature = "login-helper")]
pub mod login_helper;

#[cfg(feature = "supctl")]
pub mod supctl;

/// Description of an error from the REST APIs
#[derive(Debug, Deserialize)]
pub struct RESTError {
    /// Error message
    #[serde(rename = "error-message")]
    pub error_message: String,
    /// Additonal error information
    #[serde(rename = "error-info")]
    pub error_info: serde_json::Value,
}

/// List of REST API error messages
#[derive(Debug, Deserialize)]
pub struct RESTErrorList {
    /// Error messages
    pub errors: Vec<RESTError>,
}

/// Error returned by client functions
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Login failed
    #[error("Login failed: {0}")]
    LoginFailure(String),

    /// Application login failed because an environment variable is missing
    #[error("Login failure, missing environment variable '{0}'")]
    LoginFailureMissingEnv(String),

    /// Failed returned by the HTTP server
    #[error("HTTP failed {0}, {1} - {2}")]
    WebServer(u16, String, String),

    /// Websocket error
    #[error("Websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    /// JSON serialization/deserialization error
    #[error("Serde JSON error: {0}")]
    Serde(#[from] serde_json::Error),

    /// URL parsing error
    #[error("URL: {0}")]
    URL(#[from] url::ParseError),

    /// HTTP client error
    #[error("Reqwest: {0}")]
    HTTPClient(#[from] reqwest::Error),

    /// Volga error
    #[error("Error from Volga {0:?}")]
    Volga(Option<String>),

    /// This error is returned if we get data from the API we can't parse/understand
    #[error("API Error {0:?}")]
    API(String),

    /// This error is returned from the REST API, this typically means the client did something
    /// wrong.
    #[error("REST error {0:?}")]
    REST(RESTErrorList),

    /// TLS Errors
    #[error("TLS error {0}")]
    TLS(#[from] tokio_rustls::rustls::Error),

    /// DNSError
    #[error("DNSName error {0}")]
    DNSName(#[from] tokio_rustls::rustls::client::InvalidDnsNameError),

    /// IO Errors
    #[error("IO error {0}")]
    IO(#[from] std::io::Error),

    /// General Error
    #[error("Error {0}")]
    General(String),
}

impl Error {
    /// Create a general error
    #[must_use]
    pub fn general(err: &str) -> Self {
        Self::General(err.to_string())
    }
}

/// Result type
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LoginToken {
    token: String,
    expires_in: i64,
    expires: chrono::DateTime<chrono::FixedOffset>,
    creation_time: chrono::DateTime<chrono::FixedOffset>,
}

impl LoginToken {
    fn renew_at(&self) -> chrono::DateTime<chrono::FixedOffset> {
        self.expires - chrono::Duration::seconds(self.expires_in / 4)
    }
}

impl std::fmt::Debug for LoginToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginToken")
            .field("expires_in", &self.expires_in)
            .field("creation_time", &self.creation_time)
            .finish()
    }
}

#[derive(Debug)]
struct ClientState {
    login_token: LoginToken,
}

/// Builder for an Avassa [`Client`]
#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct ClientBuilder {
    reqwest_ca: Vec<reqwest::Certificate>,
    tls_ca: tokio_rustls::rustls::RootCertStore,
    disable_cert_verification: bool,
    connection_verbose: bool,
    auto_renew_token: bool,
    timeout: Option<core::time::Duration>,
    connect_timeout: Option<core::time::Duration>,
}

impl ClientBuilder {
    /// Create a new builder instance
    #[must_use]
    fn new() -> Self {
        Self {
            reqwest_ca: Vec::new(),
            tls_ca: tokio_rustls::rustls::RootCertStore::empty(),
            disable_cert_verification: false,
            connection_verbose: false,
            auto_renew_token: true,
            timeout: None,
            connect_timeout: None,
        }
    }

    /// Enables a request timeout
    #[must_use]
    pub fn timeout(self, timeout: core::time::Duration) -> Self {
        Self {
            timeout: Some(timeout),
            ..self
        }
    }

    /// Set a timeout for only the connect phase of a Client
    #[must_use]
    pub fn connection_timeout(self, timeout: core::time::Duration) -> Self {
        Self {
            connect_timeout: Some(timeout),
            ..self
        }
    }

    /// Add a root certificate for API certificate verification
    pub fn add_root_certificate(mut self, cert: &[u8]) -> Result<Self> {
        use std::iter;
        let r_ca = reqwest::Certificate::from_pem(cert)?;
        let mut ca_reader = std::io::BufReader::new(cert);
        for item in iter::from_fn(|| rustls_pemfile::read_one(&mut ca_reader).transpose()) {
            if let rustls_pemfile::Item::X509Certificate(cert) = item? {
                self.tls_ca.add(&tokio_rustls::rustls::Certificate(cert))?;
            }
        }
        // let t_ca = tokio_native_tls::native_tls::Certificate::from_pem(cert)?;
        self.reqwest_ca.push(r_ca);
        // self.tls_ca.push(t_ca);
        Ok(self)
    }

    /// Disable certificate verification
    #[must_use]
    pub fn danger_disable_cert_verification(self) -> Self {
        Self {
            disable_cert_verification: true,
            ..self
        }
    }

    /// Enabling this option will emit log messages at the TRACE level for read and write operations
    /// on the https client
    #[must_use]
    pub fn enable_verbose_connection(self) -> Self {
        Self {
            connection_verbose: true,
            ..self
        }
    }

    /// Disable auto renewal of authentication token
    #[must_use]
    pub fn disable_token_auto_renewal(self) -> Self {
        Self {
            auto_renew_token: false,
            ..self
        }
    }

    /// Login the application from secret set in the environment
    /// `approle_id` can optionally be provided
    /// This assumes the environment variable `APPROLE_SECRET_ID` is set by the system.
    pub async fn application_login(&self, host: &str, approle_id: Option<&str>) -> Result<Client> {
        let secret_id = std::env::var("APPROLE_SECRET_ID")
            .map_err(|_| Error::LoginFailureMissingEnv(String::from("APPROLE_SECRET_ID")))?;

        // If no app role is provided, we can try to use the secret id as app role.
        let role_id = approle_id.unwrap_or(&secret_id);

        let base_url = url::Url::parse(host)?;
        let url = base_url.join("v1/approle-login")?;
        let data = json!({
            "role-id": role_id,
            "secret-id": secret_id,
        });
        Client::do_login(self, base_url, url, data).await
    }

    /// Login to an avassa Control Tower or Edge Enforcer instance. If possible,
    /// please use the `application_login` as no credentials needs to be distributed.
    #[tracing::instrument(skip(self, password))]
    pub async fn login(&self, host: &str, username: &str, password: &str) -> Result<Client> {
        let base_url = url::Url::parse(host)?;
        let url = base_url.join("v1/login")?;

        // If we have a tenant, send it.
        let data = json!({
            "username":username,
            "password":password
        });
        Client::do_login(self, base_url, url, data).await
    }

    /// Login using an existing bearer token
    #[tracing::instrument(skip(self, token))]
    pub fn token_login(&self, host: &str, token: &str) -> Result<Client> {
        let base_url = url::Url::parse(host)?;
        Client::new_from_token(self, base_url, token)
    }
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// The `Client` is used for all interaction with Control Tower or Edge Enforcer instances.
/// Use one of the login functions to create an instance.
#[derive(Clone)]
pub struct Client {
    base_url: url::Url,
    websocket_url: url::Url,
    state: std::sync::Arc<tokio::sync::Mutex<ClientState>>,
    client: reqwest::Client,
    tls_ca: tokio_rustls::rustls::RootCertStore,
    // tls_ca: Vec<tokio_rustls::rustls::Certificate>,
    disable_cert_verification: bool,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.base_url)
            .field("websocket_url", &self.websocket_url)
            .field("state", &self.state)
            .field("client", &self.client)
            .field("disable_cert_verification", &self.disable_cert_verification)
            .finish()
    }
}

impl Client {
    /// Create a Client builder
    #[must_use]
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    async fn do_login(
        builder: &ClientBuilder,
        base_url: url::Url,
        url: url::Url,
        payload: serde_json::Value,
    ) -> Result<Self> {
        let json = serde_json::to_string(&payload)?;
        let client = Self::reqwest_client(builder)?;
        let result = client
            .post(url)
            .header("content-type", "application/json")
            .body(json)
            .send()
            .await?;

        if result.status().is_success() {
            let login_token = result.json().await?;

            Self::new(builder, client, base_url, login_token)
        } else {
            let text = result.text().await?;
            tracing::debug!("login returned {}", text);
            Err(Error::LoginFailure(text))
        }
    }

    fn reqwest_client(builder: &ClientBuilder) -> Result<reqwest::Client> {
        let reqwest_client_builder = reqwest::Client::builder();

        // Add CA certificates
        let reqwest_client_builder = builder
            .reqwest_ca
            .iter()
            .fold(reqwest_client_builder, |reqwest_client_builder, ca| {
                reqwest_client_builder.add_root_certificate(ca.clone())
            });

        let reqwest_client_builder =
            reqwest_client_builder.danger_accept_invalid_certs(builder.disable_cert_verification);

        let reqwest_client_builder =
            reqwest_client_builder.connection_verbose(builder.connection_verbose);

        let reqwest_client_builder = if let Some(duration) = builder.timeout {
            reqwest_client_builder.timeout(duration)
        } else {
            reqwest_client_builder
        };

        let reqwest_client_builder = if let Some(duration) = builder.connect_timeout {
            reqwest_client_builder.connect_timeout(duration)
        } else {
            reqwest_client_builder
        };

        let client = reqwest_client_builder.build()?;
        Ok(client)
    }

    fn new_from_token(builder: &ClientBuilder, base_url: url::Url, token: &str) -> Result<Self> {
        let client = Self::reqwest_client(builder)?;
        let creation_time = chrono::Local::now().into();
        let expires = creation_time + chrono::Duration::seconds(1);

        let login_token = LoginToken {
            token: token.to_string(),
            expires_in: 1,
            creation_time,
            expires,
        };

        Self::new(builder, client, base_url, login_token)
    }

    fn new(
        builder: &ClientBuilder,
        client: reqwest::Client,
        base_url: url::Url,
        login_token: LoginToken,
    ) -> Result<Self> {
        let websocket_url = url::Url::parse(&format!("wss://{}/v1/ws/", base_url.host_port()?))?;

        let renew_at = login_token.renew_at();

        let state = std::sync::Arc::new(tokio::sync::Mutex::new(ClientState { login_token }));

        let weak_state = std::sync::Arc::downgrade(&state);
        let refresh_url = base_url.join("/v1/state/strongbox/token/refresh")?;

        if builder.auto_renew_token {
            tokio::spawn(renew_token_task(
                weak_state,
                renew_at,
                client.clone(),
                refresh_url,
            ));
        }

        Ok(Self {
            client,
            tls_ca: builder.tls_ca.clone(),
            disable_cert_verification: builder.disable_cert_verification,
            base_url,
            websocket_url,
            state,
        })
    }

    /// Returns the login bearer token
    pub async fn bearer_token(&self) -> String {
        let state = self.state.lock().await;
        state.login_token.token.clone()
    }

    /// GET a json payload from the REST API.
    pub async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query_params: Option<&[(&str, &str)]>,
    ) -> Result<T> {
        let url = self.base_url.join(path)?;

        let token = self.bearer_token().await;

        let mut builder = self
            .client
            .get(url)
            .bearer_auth(&token)
            .header("Accept", "application/json");
        if let Some(qp) = query_params {
            builder = builder.query(qp);
        }

        let result = builder.send().await?;

        if result.status().is_success() {
            let res = result.json().await?;
            Ok(res)
        } else {
            let status = result.status();
            let error_payload = result
                .text()
                .await
                .unwrap_or_else(|_| "No error payload".to_string());
            Err(Error::WebServer(
                status.as_u16(),
                status.to_string(),
                error_payload,
            ))
        }
    }

    /// GET a bytes payload from the REST API.
    pub async fn get_bytes(
        &self,
        path: &str,
        query_params: Option<&[(&str, &str)]>,
    ) -> Result<Bytes> {
        let url = self.base_url.join(path)?;

        let token = self.bearer_token().await;

        let mut builder = self.client.get(url).bearer_auth(&token);

        if let Some(qp) = query_params {
            builder = builder.query(qp);
        }

        let result = builder.send().await?;

        if result.status().is_success() {
            let res = result.bytes().await?;
            Ok(res)
        } else {
            let status = result.status();
            let error_payload = result
                .text()
                .await
                .unwrap_or_else(|_| "No error payload".to_string());
            Err(Error::WebServer(
                status.as_u16(),
                status.to_string(),
                error_payload,
            ))
        }
    }

    /// POST arbitrary JSON to a path
    /// # Panics
    /// never
    pub async fn post_json(
        &self,
        path: &str,
        data: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let url = self.base_url.join(path)?;
        let token = self.bearer_token().await;

        tracing::debug!("POST {} {:?}", url, data);

        let result = self
            .client
            .post(url)
            .json(&data)
            .bearer_auth(&token)
            .send()
            .await?;

        if result.status().is_success() {
            let resp = result.bytes().await?;

            let mut responses: Vec<serde_json::Value> = Vec::new();
            let decoder = serde_json::Deserializer::from_slice(&resp);

            for v in decoder.into_iter() {
                responses.push(v?);
            }

            match responses.len() {
                0 => Ok(serde_json::Value::Object(serde_json::Map::default())),
                1 => Ok(responses.into_iter().next().unwrap()),
                _ => {
                    // Convert to a JSON array
                    Ok(serde_json::Value::Array(responses))
                }
            }
        } else {
            tracing::error!("POST call failed");
            let status = result.status();
            let resp = result.json().await;
            match resp {
                Ok(resp) => Err(Error::REST(resp)),
                Err(_) => Err(Error::WebServer(
                    status.as_u16(),
                    status.to_string(),
                    "Failed to get JSON responses".to_string(),
                )),
            }
        }
    }

    /// PUT arbitrary JSON to a path
    pub async fn put_json(
        &self,
        path: &str,
        data: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let url = self.base_url.join(path)?;
        let token = self.state.lock().await.login_token.token.clone();

        tracing::debug!("PUT {} {:?}", url, data);

        let result = self
            .client
            .put(url)
            .json(&data)
            .bearer_auth(&token)
            .send()
            .await?;

        #[allow(clippy::redundant_closure_for_method_calls)]
        if result.status().is_success() {
            use std::error::Error;
            let resp = result.json().await.or_else(|e| match e {
                e if e.is_decode() => {
                    match e
                        .source()
                        .and_then(|e| e.downcast_ref::<serde_json::Error>())
                    {
                        Some(e) if e.is_eof() => {
                            Ok(serde_json::Value::Object(serde_json::Map::new()))
                        }
                        _ => Err(e),
                    }
                }
                e => Err(e),
            })?;
            Ok(resp)
        } else {
            tracing::error!("PUT call failed");
            let status = result.status();
            let resp = result.json().await;
            match resp {
                Ok(resp) => Err(Error::REST(resp)),
                Err(_) => Err(Error::WebServer(
                    status.as_u16(),
                    status.to_string(),
                    "Failed to get JSON reply".to_string(),
                )),
            }
        }
    }

    /// Open a volga producer on a topic
    pub async fn volga_open_producer(
        &self,
        producer_name: &str,
        topic: &str,
        on_no_exists: volga::OnNoExists,
    ) -> Result<volga::producer::Producer> {
        crate::volga::producer::Builder::new(self, producer_name, topic, on_no_exists)?
            .connect()
            .await
    }

    /// Open a volga NAT producer on a topic in a site
    pub async fn volga_open_child_site_producer(
        &self,
        producer_name: &str,
        topic: &str,
        site: &str,
        on_no_exists: volga::OnNoExists,
    ) -> Result<volga::producer::Producer> {
        crate::volga::producer::Builder::new_child(self, producer_name, topic, site, on_no_exists)?
            .connect()
            .await
    }

    /// Creates and opens a Volga consumer
    #[tracing::instrument]
    pub async fn volga_open_consumer(
        &self,
        consumer_name: &str,
        topic: &str,
        options: crate::volga::consumer::Options,
    ) -> Result<volga::consumer::Consumer> {
        crate::volga::consumer::Builder::new(self, consumer_name, topic)?
            .set_options(options)
            .connect()
            .await
    }

    /// Creates and opens a Volga consumer on a child site
    pub async fn volga_open_child_site_consumer(
        &self,
        consumer_name: &str,
        topic: &str,
        site: &str,
        options: crate::volga::consumer::Options,
    ) -> Result<volga::consumer::Consumer> {
        crate::volga::consumer::Builder::new_child(self, consumer_name, topic, site)?
            .set_options(options)
            .connect()
            .await
    }

    #[tracing::instrument(skip(self))]
    pub(crate) async fn open_tls_stream(
        &self,
    ) -> Result<tokio_rustls::client::TlsStream<tokio::net::TcpStream>> {
        let mut client_config = tokio_rustls::rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(self.tls_ca.clone())
            .with_no_client_auth();

        if self.disable_cert_verification {
            let mut danger = client_config.dangerous();

            danger.set_certificate_verifier(std::sync::Arc::new(CertificateVerifier));
        }

        let client_config = std::sync::Arc::new(client_config);

        let connector: tokio_rustls::TlsConnector = client_config.into();
        let addrs = self.websocket_url.socket_addrs(|| None)?;
        let stream = tokio::net::TcpStream::connect(&*addrs).await?;

        let server_name =
            tokio_rustls::rustls::ServerName::try_from(self.websocket_url.host_str().unwrap())?;
        let stream = connector.connect(server_name, stream).await?;
        Ok(stream)
    }

    /// Opens a query stream
    pub async fn volga_open_log_query(
        &self,
        query: &volga::log_query::Query,
    ) -> Result<volga::log_query::QueryStream> {
        volga::log_query::QueryStream::new(self, query).await
    }

    /// Try to open a Strongbox Vault
    pub async fn open_strongbox_vault(&self, vault: &str) -> Result<strongbox::Vault> {
        strongbox::Vault::open(self, vault).await
    }
}

struct CertificateVerifier;

impl tokio_rustls::rustls::client::ServerCertVerifier for CertificateVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &tokio_rustls::rustls::Certificate,
        _intermediates: &[tokio_rustls::rustls::Certificate],
        _server_name: &tokio_rustls::rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> std::result::Result<
        tokio_rustls::rustls::client::ServerCertVerified,
        tokio_rustls::rustls::Error,
    > {
        Ok(tokio_rustls::rustls::client::ServerCertVerified::assertion())
    }
}

pub(crate) trait URLExt {
    fn host_port(&self) -> std::result::Result<String, url::ParseError>;
}

impl URLExt for url::Url {
    fn host_port(&self) -> std::result::Result<String, url::ParseError> {
        let host = self.host_str().ok_or(url::ParseError::EmptyHost)?;
        Ok(match (host, self.port()) {
            (host, Some(port)) => format!("{host}:{port}"),
            (host, _) => host.to_string(),
        })
    }
}

#[tracing::instrument(skip(next_renew_at, weak_state, client, refresh_url))]
async fn renew_token_task(
    weak_state: std::sync::Weak<tokio::sync::Mutex<ClientState>>,
    mut next_renew_at: chrono::DateTime<chrono::FixedOffset>,
    client: reqwest::Client,
    refresh_url: url::Url,
) {
    loop {
        let now: chrono::DateTime<_> = chrono::Local::now().into();
        let sleep_time = next_renew_at - now;

        tracing::debug!("renew token in {sleep_time}");

        tokio::time::sleep(
            sleep_time
                .to_std()
                .unwrap_or_else(|_| std::time::Duration::from_secs(0)),
        )
        .await;

        if let Some(state) = weak_state.upgrade() {
            let mut state = state.lock().await;
            let response = client
                .post(refresh_url.clone())
                .bearer_auth(&state.login_token.token)
                .send()
                .await;

            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Failed to renew token: {e}");
                    let now: chrono::DateTime<chrono::FixedOffset> = chrono::Local::now().into();
                    next_renew_at = now + chrono::Duration::seconds(1);
                    continue;
                }
            };

            let text = response.text().await.unwrap();
            let new_login_token = serde_json::from_str::<LoginToken>(&text);

            match new_login_token {
                Ok(new_login_token) => {
                    next_renew_at = new_login_token.renew_at();
                    state.login_token = new_login_token;
                    tracing::debug!("Successfully renewed token");
                }
                Err(e) => {
                    tracing::error!("Failed to parse or get token: {e}");
                    // After failure, we check every second
                    let now: chrono::DateTime<chrono::FixedOffset> = chrono::Local::now().into();
                    next_renew_at = now + chrono::Duration::seconds(1);
                }
            }
        } else {
            tracing::info!("renew_token: State lost");
            // If we can't get the state, the client is gone and we should go as well
            break;
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn url_ext() {
        use super::URLExt;
        let url = url::Url::parse("https://1.2.3.4:5000/a/b/c").unwrap();
        let host_port = url.host_port().unwrap();
        assert_eq!(&host_port, "1.2.3.4:5000");

        let url = url::Url::parse("https://1.2.3.4/a/b/c").unwrap();
        let host_port = url.host_port().unwrap();
        assert_eq!(&host_port, "1.2.3.4");

        let url = url::Url::parse("https://www.avassa.com/a/b/c").unwrap();
        let host_port = url.host_port().unwrap();
        assert_eq!(&host_port, "www.avassa.com");

        let url = url::Url::parse("https://www.avassa.com:1234/a/b/c").unwrap();
        let host_port = url.host_port().unwrap();
        assert_eq!(&host_port, "www.avassa.com:1234");
    }
}

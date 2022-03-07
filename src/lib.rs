//!
//! Library for interacting with an Avassa system.
//!
//! ## Avassa Client
//! The first interaction is to login into the system
//! ```no_run
//! #[tokio::main]
//! async fn main() -> Result<(), avassa_client::Error> {
//!     use avassa_client::ClientBuilder;
//!
//!     // API CA certificate loaded
//!     let ca_cert = Vec::new();
//!
//!     // Use login using platform provided application token
//!     let approle_id = "secret approle id";
//!     let client = ClientBuilder::new()
//!         .add_root_certificate(&ca_cert)?
//!         .application_login("https://api.customer.net", Some(approle_id)).await?;
//!
//!     // Username and password authentication, good during the development phase
//!     let client = ClientBuilder::new()
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
//!     use avassa_client::ClientBuilder;
//!
//!     // API CA certificate loaded
//!     let ca_cert = Vec::new();
//!
//!     // Use login using platform provided application token
//!     let approle_id = "secret approle id";
//!     let client = ClientBuilder::new()
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
//!             Default::default())
//!             .await?;
//!
//!         producer.produce("test message").await?;
//!         Ok::<_, avassa_client::Error>(())
//!     });
//!
//!     let mut consumer = client.volga_open_consumer(
//!         "test-consumer",
//!         "my-topic",
//!         Default::default())
//!         .await?;
//!
//!     let message = consumer.consume().await?;
//!
//!     assert_eq!(message.payload, "test message");
//!     Ok(())
//! }
//! ```

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::cargo)]
#![allow(clippy::missing_errors_doc)]
use serde::Deserialize;
use serde_json::json;

pub mod strongbox;
pub mod volga;

#[cfg(feature = "utilities")]
pub mod utilities;

#[cfg(feature = "login-helper")]
pub mod login_helper;

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
pub enum Error {
    /// Login failed
    #[error("Login failed: {0}")]
    LoginFailure(String),

    /// Application login failed because an environment variable is missing
    #[error("Login failure, missing environment variable '{0}'")]
    LoginFailureMissingEnv(String),

    /// Failed returned by the HTTP server
    #[error("HTTP failed {0}, {1}")]
    WebServer(u16, String),

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
    TLS(#[from] tokio_native_tls::native_tls::Error),

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

#[derive(Deserialize)]
pub(crate) struct LoginToken {
    pub token: String,
    expires_in: Option<i64>,
    pub creation_time: Option<chrono::DateTime<chrono::offset::FixedOffset>>,
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
pub struct ClientBuilder {
    reqwest_ca: Vec<reqwest::Certificate>,
    tls_ca: Vec<tokio_native_tls::native_tls::Certificate>,
    disable_hostname_check: bool,
    disable_cert_verification: bool,
}

impl ClientBuilder {
    /// Create a new builder instance
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reqwest_ca: Vec::new(),
            tls_ca: Vec::new(),
            disable_cert_verification: false,
            disable_hostname_check: false,
        }
    }

    /// Add a root certificate for API certificate verification
    pub fn add_root_certificate(mut self, cert: &[u8]) -> Result<Self> {
        let r_ca = reqwest::Certificate::from_pem(cert)?;
        let t_ca = tokio_native_tls::native_tls::Certificate::from_pem(cert)?;
        self.reqwest_ca.push(r_ca);
        self.tls_ca.push(t_ca);
        Ok(self)
    }

    /// Disable certificate verification
    #[must_use]
    pub fn danger_accept_invalid_certs(self) -> Self {
        Self {
            disable_cert_verification: true,
            ..self
        }
    }

    /// Disable hostname verification
    #[must_use]
    pub fn danger_accept_invalid_hostnames(self) -> Self {
        Self {
            disable_hostname_check: true,
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
    tls_ca: Vec<tokio_native_tls::native_tls::Certificate>,
    disable_hostname_check: bool,
    disable_cert_verification: bool,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.base_url)
            .field("websocket_url", &self.websocket_url)
            .field("state", &self.state)
            .field("client", &self.client)
            .field("disable_hostname_check", &self.disable_hostname_check)
            .field("disable_cert_verification", &self.disable_cert_verification)
            .finish()
    }
}

impl Client {
    /// Create a Client builder
    #[must_use]
    pub const fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    async fn do_login(
        builder: &ClientBuilder,
        base_url: url::Url,
        url: url::Url,
        payload: serde_json::Value,
    ) -> Result<Self> {
        let json = serde_json::to_string(&payload)?;
        let client = reqwest::Client::builder();

        // Add CA certificates
        let client = builder
            .reqwest_ca
            .iter()
            .fold(client, |client, ca| client.add_root_certificate(ca.clone()));

        let client = client.danger_accept_invalid_certs(builder.disable_cert_verification);

        let client = client.build()?;
        let result = client
            .post(url)
            .header("content-type", "application/json")
            .body(json)
            .send()
            .await?;

        if result.status().is_success() {
            let text = result.text().await?;
            let login_token = serde_json::from_str::<LoginToken>(&text)?;
            Self::new(builder, client, base_url, login_token)
        } else {
            let text = result.text().await?;
            tracing::debug!("login returned {}", text);
            Err(Error::LoginFailure(text))
        }
    }

    fn new(
        builder: &ClientBuilder,
        client: reqwest::Client,
        base_url: url::Url,
        login_token: LoginToken,
    ) -> Result<Self> {
        let websocket_url = url::Url::parse(&format!("wss://{}/v1/ws/", base_url.host_port()?))?;

        let state = ClientState { login_token };

        Ok(Self {
            client,
            tls_ca: builder.tls_ca.clone(),
            disable_cert_verification: builder.disable_cert_verification,
            disable_hostname_check: builder.disable_hostname_check,
            base_url,
            websocket_url,
            state: std::sync::Arc::new(tokio::sync::Mutex::new(state)),
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

        let token = self.state.lock().await.login_token.token.clone();

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
            Err(Error::WebServer(
                result.status().as_u16(),
                result.status().to_string(),
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
        let token = self.state.lock().await.login_token.token.clone();

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
                Err(_) => Err(Error::WebServer(status.as_u16(), status.to_string())),
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
                Err(_) => Err(Error::WebServer(status.as_u16(), status.to_string())),
            }
        }
    }

    /// Open a volga producer on a topic
    pub async fn volga_open_producer(
        &self,
        producer_name: &str,
        topic: &str,
        options: volga::Options,
    ) -> Result<volga::producer::Producer> {
        crate::volga::producer::Builder::new(self, producer_name, topic, options)?
            .set_options(options)
            .connect()
            .await
    }

    /// Open a volga NAT producer on a topic in a site
    pub async fn volga_open_nat_producer(
        &self,
        producer_name: &str,
        topic: &str,
        site: &str,
        options: volga::Options,
    ) -> Result<volga::producer::Producer> {
        crate::volga::producer::Builder::new_nat(self, producer_name, topic, site, options)?
            .set_options(options)
            .connect()
            .await
    }

    /// Creates and opens a Volga consumer
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

    /// Creates and opens a Volga NAT consumer
    pub async fn volga_open_nat_consumer(
        &self,
        consumer_name: &str,
        topic: &str,
        site: &str,
        options: crate::volga::consumer::Options,
    ) -> Result<volga::consumer::Consumer> {
        crate::volga::consumer::Builder::new_nat(self, consumer_name, topic, site)?
            .set_options(options)
            .connect()
            .await
    }

    pub(crate) async fn open_tls_stream(
        &self,
    ) -> Result<tokio_native_tls::TlsStream<tokio::net::TcpStream>> {
        let mut connector = tokio_native_tls::native_tls::TlsConnector::builder();

        self.tls_ca.iter().for_each(|ca| {
            connector.add_root_certificate(ca.clone());
        });
        connector
            .danger_accept_invalid_hostnames(self.disable_hostname_check)
            .danger_accept_invalid_certs(self.disable_cert_verification);
        let connector = connector.build()?;
        let connector: tokio_native_tls::TlsConnector = connector.into();
        let addrs = self.websocket_url.socket_addrs(|| None)?;
        let stream = tokio::net::TcpStream::connect(&*addrs).await?;
        let stream = connector
            .connect(self.websocket_url.as_str(), stream)
            .await?;
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

pub(crate) trait URLExt {
    fn host_port(&self) -> std::result::Result<String, url::ParseError>;
}

impl URLExt for url::Url {
    fn host_port(&self) -> std::result::Result<String, url::ParseError> {
        let host = self.host_str().ok_or(url::ParseError::EmptyHost)?;
        Ok(match (host, self.port()) {
            (host, Some(port)) => format!("{}:{}", host, port),
            (host, _) => host.to_string(),
        })
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

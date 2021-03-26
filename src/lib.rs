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
//! ### Create a Volga consumer
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
//!     let builder = client.volga_open_consumer(
//!         "test-consumer",
//!         "my-topic")
//!         ?;
//!
//!     let mut consumer = builder.connect().await?;
//!
//!     let message = consumer.consume().await?;
//!     Ok(())
//! }
//! ```

#![deny(missing_docs)]

use log::{debug, error};
use serde::Deserialize;
use serde_json::json;

// pub mod strongbox;
#[cfg(feature = "utilities")]
pub mod utilities;
pub mod volga;

/// Description of an error from the REST APIs
#[derive(Debug, Deserialize)]
pub struct RESTError {
    #[serde(rename = "error-message")]
    error_message: String,
    #[serde(rename = "error-info")]
    error_info: serde_json::Value,
}

/// List of REST API error messages
#[derive(Debug, Deserialize)]
pub struct RESTErrorList {
    errors: Vec<RESTError>,
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
    WebSocket(#[from] tungstenite::Error),

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
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Deserialize)]
pub(crate) struct LoginToken {
    pub token: String,
    expires_in: Option<i64>,
    pub creation_time: Option<chrono::DateTime<chrono::offset::FixedOffset>>,
}

impl LoginToken {
    // Still not clear what to do when a token has expired.
    // pub(crate) fn expires_in(&self) -> Option<chrono::Duration> {
    //     self.expires_in.map(chrono::Duration::seconds)
    // }
}

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
    pub fn new() -> Self {
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
    pub fn danger_accept_invalid_certs(self) -> Self {
        Self {
            disable_cert_verification: true,
            ..self
        }
    }

    /// Disable hostname verification
    pub fn danger_accept_invalid_hostnames(self) -> Self {
        Self {
            disable_hostname_check: true,
            ..self
        }
    }

    /// Login the application from secret set in the environment
    /// `approle_id` can optionally be provided
    /// `ca_certificate` optional CA certificate, in PEM format. If no CA certificate is provided,
    /// certificate verification is disabled. This should be fixed.
    pub async fn application_login(&self, host: &str, approle_id: Option<&str>) -> Result<Client> {
        let secret_id = std::env::var("APPROLE_SECRET_ID")
            .map_err(|_| Error::LoginFailureMissingEnv(String::from("APPROLE_SECRET_ID")))?;

        // If no app role is provided, we can try to use the secret id as app role.
        let role_id = approle_id.unwrap_or(&secret_id);

        let base_url = url::Url::parse(host)?;
        let url = base_url.join("v1/approle_login")?;
        let data = json!({
            "role_id": role_id,
            "secret_id": secret_id,
        });
        Client::do_login(&self, base_url, url, data).await
    }

    /// Login to an avassa Control Tower or Edge Enforcer instance. If possible,
    /// please use the application_login as no credentials needs to be distributed.
    pub async fn login(&self, host: &str, username: &str, password: &str) -> Result<Client> {
        let base_url = url::Url::parse(host)?;
        let url = base_url.join("v1/login")?;

        // If we have a tenant, send it.
        let data = json!({
            "username":username,
            "password":password
        });
        Client::do_login(&self, base_url, url, data).await
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

impl Client {
    /// Create a Client builder
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
            debug!("login returned {}", text);
            Err(Error::LoginFailure(text))
        }
    }

    fn new(
        builder: &ClientBuilder,
        client: reqwest::Client,
        base_url: url::Url,
        login_token: LoginToken,
    ) -> Result<Self> {
        let websocket_url = url::Url::parse(&format!("ws://{}/v1/ws/", base_url.host_port()?))?;

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
            let text = result.text().await?;
            let res = serde_json::from_str(&text)?;
            Ok(res)
        } else {
            error!("HTTP call failed");
            Err(Error::WebServer(
                result.status().as_u16(),
                result.status().to_string(),
            ))
        }
    }

    /// POST arbitrary JSON to a path
    pub async fn post_json(
        &self,
        path: &str,
        data: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let url = self.base_url.join(path)?;
        let token = self.state.lock().await.login_token.token.clone();

        debug!("POST {} {:?}", url, data);

        let result = self
            .client
            .post(url)
            .json(&data)
            .bearer_auth(&token)
            .send()
            .await?;

        if result.status().is_success() {
            use std::error::Error;
            let resp = result.json().await.or_else(|e| match e {
                e if e.is_decode() => {
                    match e
                        .source()
                        .map(|e| e.downcast_ref::<serde_json::Error>())
                        .flatten()
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
            error!("POST call failed");
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
    ) -> Result<volga::Producer> {
        crate::volga::ProducerBuilder::new(&self, producer_name, topic, options)?
            .set_options(options)
            .connect()
            .await
    }

    /// Open a volga NAT producer on a topic in a uDC
    pub async fn volga_open_nat_producer(
        &self,
        producer_name: &str,
        topic: &str,
        udc: &str,
        options: volga::Options,
    ) -> Result<volga::Producer> {
        crate::volga::ProducerBuilder::new_nat(&self, producer_name, topic, udc, options)?
            .set_options(options)
            .connect()
            .await
    }

    /// Creates and opens a Volga consumer
    pub fn volga_open_consumer(
        &self,
        consumer_name: &str,
        topic: &str,
    ) -> Result<volga::ConsumerBuilder> {
        crate::volga::ConsumerBuilder::new(&self, consumer_name, topic)
    }

    /// Creates and opens a Volga NAT consumer
    pub async fn volga_open_nat_consumer(
        &self,
        consumer_name: &str,
        topic: &str,
        udc: &str,
        volga_options: volga::ConsumerOptions,
    ) -> Result<volga::Consumer> {
        crate::volga::ConsumerBuilder::new_nat(&self, consumer_name, topic, udc)?
            .set_options(volga_options)
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
        let stream = connector.connect("192.168.8.11:4646", stream).await?;
        Ok(stream)
    }

    /// Opens a query stream
    pub async fn volga_open_log_query(&self, query: &volga::Query) -> Result<volga::QueryStream> {
        volga::QueryStream::new(self, query).await
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

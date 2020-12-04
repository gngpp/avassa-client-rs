//!
//! Library for interacting with an Avassa system.
//!
//! ## Avassa Client
//! The first interaction is to login into the system
//! ```no_run
//! #[tokio::main]
//! async fn main() -> Result<(), avassa_client::Error> {
//!     use avassa_client::AvassaClient;
//!
//!     let client = AvassaClient::login("https://1.2.3.4", "joe", "secret", "the-company").await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Volga
//! ### Create a Volga consumer
//! ```no_run
//! #[tokio::main]
//! async fn main() -> Result<(), avassa_client::Error> {
//!     use avassa_client::AvassaClient;
//!
//!     let client = AvassaClient::login("https://1.2.3.4", "joe", "secret", "the-company").await?;
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

use serde::Deserialize;
use serde_json::json;
use tracing::{debug, error};

pub mod strongbox;
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
    #[error("Login failed")]
    LoginFailure,

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

    /// This error is returned from the REST API
    #[error("REST error {0:?}")]
    REST(RESTErrorList),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Deserialize)]
pub(crate) struct LoginToken {
    pub token: String,
    expires_in: i64,
    pub accessor: String,
    pub creation_time: chrono::DateTime<chrono::offset::FixedOffset>,
}

impl LoginToken {
    pub(crate) fn expires_in(&self) -> chrono::Duration {
        chrono::Duration::seconds(self.expires_in)
    }
}

struct ClientState {
    login_token: LoginToken,
    token_expires_at: chrono::DateTime<chrono::offset::Local>,
}

/// The `AvassaClient` is used for all interaction with Control Tower or Edge Enforcer instances.
/// Use one of the login functions to create an instance.
#[derive(Clone)]
pub struct AvassaClient {
    base_url: url::Url,
    websocket_url: url::Url,
    state: std::sync::Arc<tokio::sync::Mutex<ClientState>>,
    client: reqwest::Client,
}

impl AvassaClient {
    #[tracing::instrument(level = "trace")]
    /// Login the application from secret set in the environment
    pub async fn application_login(host: &str) -> Result<Self> {
        let secret_id = std::env::var("APPROLE_SECRET_ID")
            .map_err(|_| Error::LoginFailureMissingEnv(String::from("APPROLE_SECRET_ID")))?;
        let role_id = std::env::var("APPROLE_ID").unwrap_or(secret_id.clone());

        let base_url = url::Url::parse(host)?;
        let url = base_url.join("v1/approle-login")?;
        let data = json!({
            "role_id": role_id,
            "secret_id": secret_id,
        });
        Self::do_login(base_url, url, data).await
    }

    #[tracing::instrument(level = "trace")]
    /// Login to an avassa Control Tower or Edge Enforcer instance
    pub async fn login(host: &str, username: &str, password: &str, tenant: &str) -> Result<Self> {
        let base_url = url::Url::parse(host)?;
        let url = base_url.join("v1/login")?;

        // If we have a tenant, send it.
        let data = json!({
            "username":username,
            "password":password,
            "tenant":tenant});
        Self::do_login(base_url, url, data).await
    }

    /// Returns the login bearer token
    pub async fn bearer_token(&self) -> String {
        let state = self.state.lock().await;
        state.login_token.token.clone()
    }

    #[tracing::instrument(level = "trace")]
    async fn do_login(
        base_url: url::Url,
        url: url::Url,
        payload: serde_json::Value,
    ) -> Result<Self> {
        let json = serde_json::to_string(&payload)?;
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()?;
        let result = client
            .post(url)
            .header("content-type", "application/json")
            .body(json)
            .send()
            .await?;

        if result.status().is_success() {
            let text = result.text().await?;
            let login_token = serde_json::from_str::<LoginToken>(&text)?;
            Self::new(client, base_url, login_token)
        } else {
            let text = result.text().await?;
            debug!("login returned {}", text);
            Err(Error::LoginFailure)
        }
    }

    fn new(client: reqwest::Client, base_url: url::Url, login_token: LoginToken) -> Result<Self> {
        let token_expires_at = chrono::Local::now() + login_token.expires_in();
        let websocket_url = url::Url::parse(&format!("ws://{}/v1/ws/", base_url.host_port()?))?;

        let state = ClientState {
            login_token,
            token_expires_at,
        };

        Ok(Self {
            client,
            base_url,
            websocket_url,
            state: std::sync::Arc::new(tokio::sync::Mutex::new(state)),
        })
    }

    /// GET a json payload from the REST API
    pub async fn get_json(
        &self,
        path: &str,
        query_params: Option<&[(&str, &str)]>,
    ) -> Result<serde_json::Value> {
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

    pub(crate) async fn post_json(
        &self,
        path: &str,
        data: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let url = self.base_url.join(path)?;
        let token = self.state.lock().await.login_token.token.clone();

        debug!("POST {} {:?}", url, data);

        println!("{}", line!());
        let result = self
            .client
            .post(url)
            .json(&data)
            .bearer_auth(&token)
            .send()
            .await?;

        if result.status().is_success() {
            let resp = result.json().await?;
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
        crate::volga::ProducerBuilder::new(&self, producer_name, topic)?
            .set_options(options)
            .connect()
            .await
    }

    /// Open a Volga infra producer
    pub async fn volga_open_infra_producer(&self, topic: &str) -> Result<volga::InfraProducer> {
        crate::volga::InfraProducerBuilder::new(&self, topic)?
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
        volga_options: volga::Options,
    ) -> Result<volga::Consumer> {
        let consumer_opts = volga::ConsumerOptions {
            volga_options,
            ..Default::default()
        };
        crate::volga::ConsumerBuilder::new_nat(&self, consumer_name, topic, udc)?
            .set_options(consumer_opts)
            .connect()
            .await
    }
}

pub(crate) trait URLExt {
    fn host_port(&self) -> std::result::Result<String, url::ParseError>;
}

impl URLExt for url::Url {
    fn host_port(&self) -> std::result::Result<String, url::ParseError> {
        //
        let port = self.port().map(|p| format!(":{}", p)).unwrap_or("".into());
        let host = self.host_str().ok_or(url::ParseError::EmptyHost)?;
        Ok(format!("{}{}", host, port))
    }
}

//!
//! Library for producing and consuming Volga messages.
//!
use crate::{Error, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

pub mod consumer;
pub mod log_query;
pub mod producer;

type WebSocketStream =
    tokio_tungstenite::WebSocketStream<tokio_native_tls::TlsStream<tokio::net::TcpStream>>;

/// Volga stream persistence
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub enum Persistence {
    /// Persist messages to disk
    #[serde(rename = "disk")]
    Disk,
    /// Store messages in RAM
    #[serde(rename = "ram")]
    RAM,
}

/// Format of the data on the volga topic
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub enum Format {
    /// JSON format
    #[serde(rename = "json")]
    JSON,
    /// String encoded
    #[serde(rename = "string")]
    String,
}

impl Default for Format {
    fn default() -> Self {
        Self::JSON
    }
}

/// Behavior when topic doesn't exist
#[derive(Clone, Copy, Debug)]
// #[serde(rename_all = "kebab-case")]
pub enum OnNoExists {
    Create(CreateOptions),
    Wait,
    Fail,
}

impl Serialize for OnNoExists {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry(
            "on-no-exists",
            match self {
                Self::Wait => "wait",
                Self::Fail => "fail",
                Self::Create(_) => "create",
            },
        )?;
        if let Self::Create(create_opts) = self {
            map.serialize_entry("create-options", create_opts)?;
        }

        map.end()
    }
}

/// Local or NAT connection
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Location {
    Local,
    ChildSite,
    Parent,
}

/// Volga options for consumers and producers
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct CreateOptions {
    /// Number of replicas in the cluster
    pub replication_factor: u32,
    /// Volga stream persistence
    pub persistence: Persistence,

    /// number of 1Mb chunks
    pub num_chunks: usize,

    /// Volga format
    pub format: Format,

    /// Delete topic after call producers and consumers disconnect
    pub ephemeral: bool,
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self {
            replication_factor: 1,
            persistence: Persistence::Disk,
            format: Format::default(),
            num_chunks: 100,
            ephemeral: false,
        }
    }
}

async fn get_binary_response(ws: &mut WebSocketStream) -> Result<Vec<u8>> {
    loop {
        let resp = ws
            .next()
            .await
            .ok_or_else(|| Error::Volga(Some("Expected websocket message".to_string())))??;

        match resp {
            tokio_tungstenite::tungstenite::Message::Pong(_) => continue,
            tokio_tungstenite::tungstenite::Message::Binary(m) => return Ok(m),
            tokio_tungstenite::tungstenite::Message::Close(_) => {
                return Err(Error::Volga(Some("closed".to_string())));
            }
            msg => {
                return Err(Error::Volga(Some(format!(
                    "Unexpected message type: '{}'",
                    msg
                ))))
            }
        }
    }
}

async fn get_ok_volga_response(ws: &mut WebSocketStream) -> Result<()> {
    let msg = get_binary_response(ws).await?;
    let resp: VolgaResponse = serde_json::from_slice(&msg)?;
    tracing::trace!("volga response {:?}", resp);
    match resp.result {
        VolgaResult::Ok => Ok(()),
        VolgaResult::Error => Err(Error::Volga(resp.info)),
    }
}

#[derive(Debug, Deserialize)]
enum VolgaResult {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Deserialize)]
struct VolgaResponse {
    result: VolgaResult,
    info: Option<String>,
}

#[cfg(test)]
mod test {
    #[test]
    fn on_no_exists() {
        let wait = serde_json::to_string(&super::OnNoExists::Wait).unwrap();
        assert_eq!(&wait, r#"{"on-no-exists":"wait"}"#);

        let fail = serde_json::to_string(&super::OnNoExists::Fail).unwrap();
        assert_eq!(&fail, r#"{"on-no-exists":"fail"}"#);

        let create = serde_json::to_string(&super::OnNoExists::Create(Default::default())).unwrap();
        assert_eq!(
            &create,
            r#"{"on-no-exists":"create","create-options":{"replication-factor":1,"persistence":"disk","num-chunks":100,"format":"json","ephemeral":false}}"#
        );
    }
}

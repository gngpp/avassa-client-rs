//!
//! Library for producing and consuming Volga messages.
//!
use crate::{Error, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

mod consumer;
mod log_query;
mod producer;

pub use consumer::*;
pub use log_query::*;
pub use producer::*;

type WebSocketStream =
    tokio_tungstenite::WebSocketStream<tokio_native_tls::TlsStream<tokio::net::TcpStream>>;

/// Volga stream persistence
#[derive(Clone, Copy, Debug, Serialize)]
pub enum Persistence {
    /// Persist messages to disk
    #[serde(rename = "disk")]
    Disk,
    /// Store messages in RAM
    #[serde(rename = "ram")]
    RAM,
}

/// Volga options for consumers and producers
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Options {
    /// Fail if the topic does not exist
    pub create: bool,
    /// Number of replicas in the cluster
    #[serde(rename = "replication-factor")]
    pub replication_factor: u32,
    /// Volga stream persistence
    pub persistence: Persistence,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            create: true,
            replication_factor: 1,
            persistence: Persistence::Disk,
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
            msg => return Err(Error::Volga(Some(format!("Unexpected: {}", msg)))),
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

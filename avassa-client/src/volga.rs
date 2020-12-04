//!
//! Library for producing and consuming Volga messages.
//!
use futures_util::{SinkExt, StreamExt};
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message as WSMessage};
use tracing::{debug, trace};

type WebSocketStream = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

const N_IN_AUTO_MORE: usize = 5;

use crate::{Error, Result};

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

/// Volga stream mode
#[derive(Clone, Copy, Debug, Serialize)]
pub enum Mode {
    /// Only a single consumer can connect
    #[serde(rename = "exclusive")]
    Exclusive,
    /// Messages are sent to consumers, with the same name, in a round-robin fashon.
    #[serde(rename = "shared")]
    Shared,
    /// Act as a backup/standby consumer
    #[serde(rename = "standby")]
    Standby,
}

/// Volga Consumer starting position
#[derive(Clone, Copy, Debug, Serialize)]
pub enum ConsumerPosition {
    /// Get all messages from the beginning
    #[serde(rename = "beginning")]
    Beginning,
    /// Get all unread messages
    #[serde(rename = "unread")]
    Unread,
}

impl Default for ConsumerPosition {
    fn default() -> Self {
        Self::Unread
    }
}

/// Volga options for consumers and producers
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Options {
    /// Fail if the topic does not exist
    pub create: bool,
    /// Number of replicas in the cluster
    pub replication_factor: u32,
    /// Volga stream persistence
    pub persistence: Persistence,
    /// Volga stream mode
    pub mode: Mode,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            create: true,
            replication_factor: 1,
            persistence: Persistence::Disk,
            mode: Mode::Exclusive,
        }
    }
}

async fn get_binary_response(ws: &mut WebSocketStream) -> Result<Vec<u8>> {
    loop {
        let resp = ws
            .next()
            .await
            .ok_or(Error::Volga(Some("Expected websocket message".to_string())))??;

        match resp {
            tokio_tungstenite::tungstenite::Message::Pong(_) => continue,
            tokio_tungstenite::tungstenite::Message::Binary(m) => return Ok(m),
            msg => return Err(Error::Volga(Some(format!("Unexpected: {}", msg)))),
        }
    }
}

#[tracing::instrument(level = "debug", skip(ws))]
async fn get_ok_volga_response(ws: &mut WebSocketStream) -> Result<()> {
    let msg = get_binary_response(ws).await?;
    let resp: VolgaResponse = serde_json::from_slice(&msg)?;
    match resp.result {
        VolgaResult::Ok => Ok(()),
        VolgaResult::Error => Err(Error::Volga(resp.info)),
    }
}

/// [`Consumer`] options
#[derive(Debug)]
pub struct ConsumerOptions {
    /// Volga general options
    pub volga_options: Options,
    /// Starting position
    pub position: ConsumerPosition,
    /// If set, the client will automatically request more items
    pub auto_more: bool,
}

impl Default for ConsumerOptions {
    fn default() -> Self {
        Self {
            volga_options: Default::default(),
            position: Default::default(),
            auto_more: true,
        }
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

/// [`Consumer`] builder
pub struct ConsumerBuilder<'a> {
    avassa_client: &'a crate::AvassaClient,
    volga_url: url::Url,
    ws_url: url::Url,
    name: String,
    options: ConsumerOptions,
}

/// Created from the Avassa Client.
impl<'a> ConsumerBuilder<'a> {
    /// Create a Volga Consumer Builder
    pub(crate) fn new(
        avassa_client: &'a crate::AvassaClient,
        name: &str,
        topic: &str,
    ) -> Result<Self> {
        // let hp = avassa_client
        //     .base_url
        //     .host()
        //     .ok_or(url::ParseError::EmptyHost)?;
        let hp = "localhost";
        let volga_url = url::Url::parse(&format!("volga://{}/{}", hp, topic,))?;

        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            volga_url,
            ws_url,
            name: name.to_owned(),
            options: Default::default(),
        })
    }

    /// Create a Volga NAT Consumer Builder
    pub(crate) fn new_nat(
        avassa_client: &'a crate::AvassaClient,
        name: &str,
        topic: &str,
        udc: &str,
    ) -> Result<Self> {
        let volga_url = url::Url::parse(&format!("volga-nat://{}/{}", udc, topic,))?;

        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            volga_url,
            ws_url,
            name: name.to_owned(),
            options: Default::default(),
        })
    }

    /// Set Volga `ConsumerOptions`
    pub fn set_options(self, options: ConsumerOptions) -> Self {
        Self { options, ..self }
    }

    /// Connect and create a `Consumer`
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn connect(self) -> Result<Consumer> {
        let request = tungstenite::handshake::client::Request::builder()
            .uri(self.ws_url.to_string())
            .header(
                "Authorization",
                format!("Bearer {}", self.avassa_client.bearer_token().await),
            )
            .body(())
            .map_err(tungstenite::error::Error::HttpFormat)?;
        let (mut ws, _) = connect_async(request).await?;

        let cmd = json!({
            "op": "open_consumer",
            "url": self.volga_url.as_str(),
            "name": self.name,
            "position": self.options.position,
            "opts": self.options.volga_options,
        });

        let json = serde_json::to_string_pretty(&cmd)?;
        debug!("{}", json);

        ws.send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;

        get_ok_volga_response(&mut ws).await?;

        debug!("Successfully conected consumer to {}", self.volga_url);
        let mut consumer = Consumer {
            ws,
            options: self.options,
            last_seq_no: 0,
            // last_timestamp: chrono::Utc::now(),
        };

        if consumer.options.auto_more {
            consumer.more(N_IN_AUTO_MORE).await?;
        }

        Ok(consumer)
    }
}

#[derive(Debug, Deserialize)]
struct Message {
    /// Consumer name
    pub name: String,
    /// True if a control message
    pub control: bool,
    /// Volga message payload
    pub payload: Option<String>,
    /// Timestamp (seconds since epoch)
    pub time: u64,
    /// Sequence number
    pub seqno: u64,
    /// The number of remaining message the client has indicated it can handle,
    /// see the [Consumer more](struct.Consumer.html) function.
    pub remain: usize,
}

/// Volga Consumer
#[pin_project]
pub struct Consumer {
    ws: WebSocketStream,
    options: ConsumerOptions,
    last_seq_no: u64,
    // last_timestamp: chrono::DateTime<chrono::Utc>,
}

impl Consumer {
    /// Indicate the client is ready for n more messages. If auto_more is set in the
    /// options, this is automatically handled.
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn more(&mut self, n: usize) -> Result<()> {
        let cmd = json!( {
            "op": "more",
            "n": n,
        });

        debug!("{}", cmd);
        self.ws
            .send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;

        Ok(())
    }

    /// Wait for the next message from Volga
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn consume(&mut self) -> Result<Vec<u8>> {
        let timeout = std::time::Duration::from_secs(20);

        loop {
            // First get the control message
            match tokio::time::timeout(timeout, get_binary_response(&mut self.ws)).await {
                Err(_) => {
                    trace!("Sending ping");
                    let ping = tokio_tungstenite::tungstenite::Message::Ping(vec![0; 1]);
                    self.ws.send(ping).await?;
                }
                Ok(msg) => {
                    let msg = msg?;
                    let resp: Message = serde_json::from_slice(&msg)?;
                    self.last_seq_no = resp.seqno;
                    // self.last_timestamp=

                    // Read the actual message
                    let msg = get_binary_response(&mut self.ws).await?;

                    if resp.remain == 0 && self.options.auto_more {
                        self.more(N_IN_AUTO_MORE).await?;
                    }
                    return Ok(msg);
                }
            }
        }
    }

    /// Wait for a message for a maximum time. At timeout, None is returned.
    pub async fn consume_with_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<Option<Vec<u8>>> {
        match tokio::time::timeout(timeout, self.consume()).await {
            Err(_) => Ok(None),
            Ok(v) => v.map(Option::Some),
        }
    }

    /// returns the last received sequence number
    pub fn last_seq_no(&self) -> u64 {
        self.last_seq_no
    }
}

// impl Stream for Consumer {
//     type Item = Result<Vec<u8>>;

//     fn poll_next(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Option<Self::Item>> {
//         let this = self.project();
//         match this.ws.poll_next(cx) {
//             Some(Ok(val)) => {
//                 todo!()
//             }
//             o => return o,
//         }
//     }
// }

/// [`Producer`] builder
pub struct ProducerBuilder<'a> {
    avassa_client: &'a super::AvassaClient,
    volga_url: url::Url,
    ws_url: url::Url,
    name: &'a str,
    options: Options,
}

impl<'a> ProducerBuilder<'a> {
    /// Create new Producer builder
    pub fn new(avassa_client: &'a super::AvassaClient, name: &'a str, topic: &str) -> Result<Self> {
        let hp = "localhost";
        let volga_url = url::Url::parse(&format!("volga://{}/{}", hp, topic,))?;

        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            volga_url,
            ws_url,
            name,
            options: Options::default(),
        })
    }

    /// Set Volga [`Options`]
    pub fn set_options(self, options: Options) -> Self {
        Self { options, ..self }
    }

    /// Connect and create a [`Producer`]
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn connect(self) -> Result<Producer> {
        let request = tungstenite::handshake::client::Request::builder()
            .uri(self.ws_url.to_string())
            .header(
                "Authorization",
                format!("Bearer {}", self.avassa_client.bearer_token().await),
            )
            .body(())
            .map_err(tungstenite::error::Error::HttpFormat)?;

        let (mut ws, _) = connect_async(request).await?;

        let cmd = json!({
            "op": "open_producer",
            "url": self.volga_url.as_str(),
            "name": self.name,
            "opts": self.options,
        });

        let json = serde_json::to_string_pretty(&cmd)?;
        debug!("{}", json);

        ws.send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;

        debug!("Waiting for ok");
        // let resp = ws.next().await;
        // debug!("resp: {:?}", resp);
        get_ok_volga_response(&mut ws).await?;
        debug!("Successfully connected producer {}", self.volga_url);
        Ok(Producer { ws })
    }
}

/// Volga Producer
pub struct Producer {
    ws: WebSocketStream,
}
impl Producer {
    /// Produce message
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn produce(&mut self, content: String) -> Result<()> {
        let cmd = json!({
            "op": "produce",
            "mode": "sync",
            "payload": content,
        });

        debug!("Cmd: {}", cmd);

        self.ws
            .send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;

        Ok(())
    }
}

/// Direction for passing of messages in Volga Infra
#[derive(Debug, Serialize)]
pub enum InfraDirection {
    /// Pass downstream
    #[serde(rename = "down")]
    Down,
    /// Pass upstream
    #[serde(rename = "up")]
    Up,
    /// Pass both up and down
    #[serde(rename = "both")]
    Both,
    /// Pass messages using a stitching data center
    #[serde(rename = "stitch")]
    Stitch,
}

/// [`InfraProducer`] builder
pub struct InfraProducerBuilder<'a> {
    avassa_client: &'a super::AvassaClient,
    topic: String,
    ws_url: url::Url,
}

impl<'a> InfraProducerBuilder<'a> {
    /// Create a new Volga Infra producer builder
    pub fn new(avassa_client: &'a super::AvassaClient, topic: &str) -> Result<Self> {
        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            topic: topic.to_string(),
            ws_url,
        })
    }

    /// Try to connect and create the producer
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn connect(self) -> Result<InfraProducer> {
        let request = tungstenite::handshake::client::Request::builder()
            .uri(self.ws_url.to_string())
            .header(
                "Authorization",
                format!("Bearer {}", self.avassa_client.bearer_token().await),
            )
            .body(())
            .map_err(tungstenite::error::Error::HttpFormat)?;

        let (mut ws, _) = connect_async(request).await?;

        let cmd = json!({
            "op": "infra_get_producer_handle",
            "topic": self.topic.as_str(),
        });

        let json = serde_json::to_string_pretty(&cmd)?;
        debug!("{}", json);

        ws.send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;

        get_ok_volga_response(&mut ws).await?;

        debug!("Successfully connected infra producer {}", self.topic);
        Ok(InfraProducer { ws })
    }
}

/// Volga Infra Producer
pub struct InfraProducer {
    ws: WebSocketStream,
}

impl InfraProducer {
    /// Send message
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn produce(&mut self, content: String) -> Result<()> {
        let cmd = json!({
            "op": "infra_produce",
            "direction": InfraDirection::Down,
            "payload": content,
        });

        debug!("Cmd: {}", cmd);

        self.ws
            .send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;

        Ok(())
    }
}

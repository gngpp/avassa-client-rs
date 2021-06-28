use super::{Options, WebSocketStream};
use crate::Result;
use chrono::serde::ts_milliseconds;
use chrono::{DateTime, Utc};
use futures_util::SinkExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::{client_async, tungstenite::Message as WSMessage};

const N_IN_AUTO_MORE: usize = 5;

/// [`Consumer`] options
#[derive(Clone, Copy, Debug)]
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
            volga_options: super::Options {
                // As consumer, try with create false.
                create: false,
                ..Default::default()
            },
            position: Default::default(),
            auto_more: true,
        }
    }
}

/// Volga Consumer starting position
#[derive(Clone, Copy, Debug, Serialize)]
pub enum ConsumerPosition {
    /// Get all messages from the beginning
    #[serde(rename = "beginning")]
    Beginning,
    /// Start consuming from the end, i.e only get new messages
    #[serde(rename = "end")]
    End,
    /// Get all unread messages
    #[serde(rename = "unread")]
    Unread,
    /// Start consuming from a sequence number
    #[serde(skip)]
    SequenceNumber(u64),

    /// Start consuming from a timestamp
    #[serde(skip)]
    TimeStamp(chrono::DateTime<chrono::Local>),
}

impl Default for ConsumerPosition {
    fn default() -> Self {
        Self::Unread
    }
}

/// [`Consumer`] builder
pub struct ConsumerBuilder<'a> {
    avassa_client: &'a crate::Client,
    volga_url: url::Url,
    ws_url: url::Url,
    name: String,
    options: ConsumerOptions,
}

/// Created from the Avassa Client.
impl<'a> ConsumerBuilder<'a> {
    /// Create a Volga Consumer Builder
    pub(crate) fn new(avassa_client: &'a crate::Client, name: &str, topic: &str) -> Result<Self> {
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
        avassa_client: &'a crate::Client,
        name: &str,
        topic: &str,
        site: &str,
    ) -> Result<Self> {
        let volga_url = url::Url::parse(&format!("volga-nat://{}/{}", site, topic,))?;

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
    pub async fn connect(self) -> Result<Consumer> {
        let request = tungstenite::handshake::client::Request::builder()
            .uri(self.ws_url.to_string())
            .header(
                "Authorization",
                format!("Bearer {}", self.avassa_client.bearer_token().await),
            )
            .body(())
            .map_err(tungstenite::error::Error::HttpFormat)?;
        let tls = self.avassa_client.open_tls_stream().await?;
        let (mut ws, _) = client_async(request, tls).await?;

        let cmd = match self.options.position {
            super::ConsumerPosition::SequenceNumber(seqno) => json!({
                "op": "open-consumer",
                "url": self.volga_url.as_str(),
                "name": self.name,
                "position": "seqno",
                "position-sequence-number": seqno,
                "opts": self.options.volga_options,
            }),
            super::ConsumerPosition::TimeStamp(ts) => json!({
                "op": "open-consumer",
                "url": self.volga_url.as_str(),
                "name": self.name,
                "position": "timestamp",
                "position-timestamp": ts,
                "opts": self.options.volga_options,
            }),
            _ => json!({
                "op": "open-consumer",
                "url": self.volga_url.as_str(),
                "name": self.name,
                "position": self.options.position,
                "opts": self.options.volga_options,
            }),
        };

        let json = serde_json::to_string_pretty(&cmd)?;
        tracing::debug!("{}", json);

        ws.send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;

        super::get_ok_volga_response(&mut ws).await?;

        tracing::debug!("Successfully conected consumer to {}", self.volga_url);
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

/// Metadata on the Volga message received in 'Consumer::consume'
#[derive(Debug, Deserialize)]
pub struct MessageMetadata {
    /// Consumer name
    pub name: String,
    /// True if a control message
    pub control: bool,
    /// Timestamp (milliseconds since epoch)
    #[serde(with = "ts_milliseconds")]
    pub time: DateTime<Utc>,
    /// Sequence number
    pub seqno: u64,
    /// The number of remaining message the client has indicated it can handle,
    /// see the [Consumer more](struct.Consumer.html) function.
    pub remain: u64,
    /// The message payload
    pub payload: String,
}

/// Volga Consumer
pub struct Consumer {
    ws: WebSocketStream,
    options: ConsumerOptions,
    last_seq_no: u64,
    // last_timestamp: chrono::DateTime<chrono::Utc>,
}

impl Consumer {
    /// Indicate the client is ready for n more messages. If auto_more is set in the
    /// options, this is automatically handled.
    pub async fn more(&mut self, n: usize) -> Result<()> {
        let cmd = json!( {
            "op": "more",
            "n": n,
        });

        tracing::trace!("{}", cmd);
        self.ws
            .send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;

        Ok(())
    }

    /// Wait for the next message from Volga
    pub async fn consume(&mut self) -> Result<MessageMetadata> {
        let timeout = std::time::Duration::from_secs(20);

        loop {
            match tokio::time::timeout(timeout, super::get_binary_response(&mut self.ws)).await {
                Err(_) => {
                    tracing::trace!("Sending ping");
                    let ping = tokio_tungstenite::tungstenite::Message::Ping(vec![0; 1]);
                    self.ws.send(ping).await?;
                }
                Ok(msg) => {
                    let msg = msg?;
                    tracing::info!("msg: '{}'", String::from_utf8_lossy(&msg));
                    let resp: MessageMetadata = serde_json::from_slice(&msg)?;
                    self.last_seq_no = resp.seqno;
                    tracing::trace!("Metadata: {:?}", resp);

                    if resp.remain == 0 && self.options.auto_more {
                        self.more(N_IN_AUTO_MORE).await?;
                    }
                    return Ok(resp);
                }
            }
        }
    }

    /// returns the last received sequence number
    pub fn last_seq_no(&self) -> u64 {
        self.last_seq_no
    }
}

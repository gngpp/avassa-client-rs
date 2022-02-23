use super::WebSocketStream;
use crate::Result;
use chrono::{DateTime, Utc};
use futures_util::SinkExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::{client_async, tungstenite::Message as WSMessage};

const N_IN_AUTO_MORE: usize = 5;

/// Volga stream mode
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub enum Mode {
    /// Consumer names has to be unique
    #[serde(rename = "exclusive")]
    Exclusive,
    /// Messages are sent to consumers, with the same name, in a round-robin fashon.
    #[serde(rename = "shared")]
    Shared,
    /// Act as a backup/standby consumer
    #[serde(rename = "standby")]
    Standby,
}

/// [`Consumer`] options
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Volga general options
    pub volga_options: crate::volga::Options,

    /// Starting position
    pub position: Position,

    /// If set, the client will automatically request more items
    pub auto_more: bool,

    /// Volga stream mode
    pub mode: Mode,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct OpenConsumer<'a> {
    op: &'a str,
    location: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "nat-site")]
    nat_site: Option<&'a str>,
    topic: &'a str,
    name: &'a str,
    position: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "position-sequence-number")]
    position_sequence_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "position-timestamp")]
    position_timestamp: Option<chrono::DateTime<chrono::Local>>,
    opts: crate::volga::Options,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            volga_options: super::Options {
                // As consumer, try with create false.
                create: false,
                ..crate::volga::Options::default()
            },
            position: Position::default(),
            auto_more: true,
            mode: Mode::Exclusive,
        }
    }
}

/// Volga Consumer starting position
#[derive(Clone, Copy, Debug, Serialize)]
pub enum Position {
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

impl Default for Position {
    fn default() -> Self {
        Self::End
    }
}

/// [`Consumer`] builder
pub struct Builder<'a> {
    avassa_client: &'a crate::Client,
    location: &'a str,
    nat_site: Option<&'a str>,
    topic: &'a str,
    ws_url: url::Url,
    name: &'a str,
    options: Options,
}

/// Created from the Avassa Client.
impl<'a> Builder<'a> {
    /// Create a Volga Consumer Builder
    pub(crate) fn new(
        avassa_client: &'a crate::Client,
        name: &'a str,
        topic: &'a str,
    ) -> Result<Self> {
        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            location: "local",
            nat_site: None,
            topic,
            ws_url,
            name,
            options: crate::volga::consumer::Options::default(),
        })
    }

    /// Create a Volga NAT Consumer Builder
    pub(crate) fn new_nat(
        avassa_client: &'a crate::Client,
        name: &'a str,
        topic: &'a str,
        site: &'a str,
    ) -> Result<Self> {
        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            location: "nat-site",
            nat_site: Some(site),
            topic,
            ws_url,
            name,
            options: crate::volga::consumer::Options::default(),
        })
    }

    /// Set Volga `Options`
    #[must_use]
    pub fn set_options(self, options: Options) -> Self {
        Self { options, ..self }
    }

    /// Connect and create a `Consumer`
    pub async fn connect(self) -> Result<Consumer> {
        let request = tokio_tungstenite::tungstenite::handshake::client::Request::builder()
            .uri(self.ws_url.to_string())
            .header(
                "Authorization",
                format!("Bearer {}", self.avassa_client.bearer_token().await),
            )
            .body(())
            .map_err(tokio_tungstenite::tungstenite::error::Error::HttpFormat)?;
        let tls = self.avassa_client.open_tls_stream().await?;
        let (mut ws, _) = client_async(request, tls).await?;
        let cmd = OpenConsumer {
            op: "open-consumer",
            location: self.location,
            nat_site: self.nat_site,
            topic: self.topic,
            name: self.name,
            position: match self.options.position {
                Position::SequenceNumber(_seqno) => "seqno",
                Position::TimeStamp(_ts) => "timestamp",
                Position::Beginning => "beginning",
                Position::End => "end",
                Position::Unread => "unread",
            },
            position_sequence_number: match self.options.position {
                Position::SequenceNumber(seqno) => Some(seqno),
                _ => None,
            },
            position_timestamp: match self.options.position {
                Position::TimeStamp(ts) => Some(ts),
                _ => None,
            },
            opts: self.options.volga_options,
        };

        tracing::debug!("{:?}", serde_json::to_string_pretty(&cmd));

        ws.send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;

        super::get_ok_volga_response(&mut ws).await?;

        tracing::debug!("Successfully connected consumer to topic {}", self.topic);
        let mut consumer = Consumer {
            ws,
            options: self.options,
            last_seq_no: 0,
        };

        if consumer.options.auto_more {
            consumer.more(N_IN_AUTO_MORE).await?;
        }

        Ok(consumer)
    }
}

/// Metadata on the Volga message received in `Consumer::consume`
#[derive(Debug, Serialize, Deserialize)]
pub struct MessageMetadata {
    /// Consumer name
    pub name: String,

    /// Timestamp
    pub time: DateTime<Utc>,

    /// Milliseconds since epoch
    pub mtime: u64,

    /// Sequence number
    pub seqno: u64,

    /// The number of remaining message the client has indicated it can handle,
    /// see the [Consumer more](struct.Consumer.html) function.
    pub remain: u64,

    /// The message payload
    pub payload: serde_json::Value,
}

/// Volga Consumer
pub struct Consumer {
    ws: WebSocketStream,
    options: Options,
    last_seq_no: u64,
}

impl Consumer {
    /// Indicate the client is ready for n more messages. If `auto_more` is set in the
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
            // FIXME: should not have to send pings.
            match tokio::time::timeout(timeout, super::get_binary_response(&mut self.ws)).await {
                Err(_) => {
                    tracing::trace!("Sending ping");
                    let ping = tokio_tungstenite::tungstenite::Message::Ping(vec![0; 1]);
                    self.ws.send(ping).await?;
                }
                Ok(msg) => {
                    let msg = msg?;
                    // tracing::error!("{}", String::from_utf8_lossy(&msg));

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
    #[must_use]
    pub const fn last_seq_no(&self) -> u64 {
        self.last_seq_no
    }
}

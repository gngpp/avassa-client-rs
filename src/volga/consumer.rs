use super::WebSocketStream;
use crate::Result;
use chrono::{DateTime, Utc};
use futures_util::SinkExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message as WSMessage};

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
    /// Starting position
    pub position: Position,

    /// If set, the client will automatically request more items
    pub auto_more: bool,

    /// Volga stream mode
    pub mode: Mode,

    /// Optional create options
    pub on_no_exists: super::OnNoExists,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct OpenConsumer<'a> {
    op: &'a str,
    location: super::Location,
    #[serde(skip_serializing_if = "Option::is_none")]
    child_site: Option<&'a str>,
    topic: &'a str,
    name: &'a str,
    position: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    position_sequence_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position_timestamp: Option<chrono::DateTime<chrono::Local>>,
    #[serde(flatten)]
    on_no_exists: super::OnNoExists,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            on_no_exists: super::OnNoExists::Wait,
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
    location: super::Location,
    child_site: Option<&'a str>,
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
            location: super::Location::Local,
            child_site: None,
            topic,
            ws_url,
            name,
            options: crate::volga::consumer::Options::default(),
        })
    }

    /// Create a Volga NAT Consumer Builder
    pub(crate) fn new_child(
        avassa_client: &'a crate::Client,
        name: &'a str,
        topic: &'a str,
        site: &'a str,
    ) -> Result<Self> {
        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            location: super::Location::ChildSite,
            child_site: Some(site),
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
        let mut request = self.ws_url.into_client_request()?;
        request.headers_mut().insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!(
                "Bearer {}",
                self.avassa_client.bearer_token().await
            ))
            .map_err(|_e| {
                crate::Error::General("Failed to set Authorization header".to_string())
            })?,
        );
        let tls = self.avassa_client.open_tls_stream().await?;
        let (mut ws, _) = tokio_tungstenite::client_async(request, tls).await?;
        let cmd = OpenConsumer {
            op: "open-consumer",
            location: self.location,
            child_site: self.child_site,
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
            on_no_exists: self.options.on_no_exists,
        };

        tracing::debug!("{:#?}", serde_json::to_string_pretty(&cmd));

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
#[serde(rename_all = "kebab-case")]
pub struct MessageMetadata {
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

    /// Name of the producer
    pub producer_name: Option<String>,
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
    #[tracing::instrument(skip(self), level = "debug")]
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
    #[tracing::instrument(skip(self), level = "trace")]
    pub async fn consume(&mut self) -> Result<MessageMetadata> {
        let msg = super::get_binary_response(&mut self.ws).await?;
        tracing::trace!("message: {}", String::from_utf8_lossy(&msg));

        let resp: MessageMetadata = serde_json::from_slice(&msg)?;
        self.last_seq_no = resp.seqno;
        tracing::trace!("Metadata: {:?}", resp);

        if resp.remain == 0 && self.options.auto_more {
            self.more(N_IN_AUTO_MORE).await?;
        }
        Ok(resp)
    }

    /// Wait for the next message from Volga
    #[tracing::instrument(skip(self))]
    async fn consume_with_ping(&mut self) -> Result<MessageMetadata> {
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
                    tracing::trace!("message: {}", String::from_utf8_lossy(&msg));

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

    pub async fn ack(&mut self, seqno: u64) -> Result<()> {
        tracing::trace!("ack: {}", seqno);
        let cmd = serde_json::json!({
            "op": "ack",
            "seqno": seqno,
        });

        self.ws
            .send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;
        Ok(())
    }

    /// returns the last received sequence number
    #[must_use]
    pub const fn last_seq_no(&self) -> u64 {
        self.last_seq_no
    }
}

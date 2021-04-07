use super::{Options, WebSocketStream};
use crate::Result;
use chrono::serde::ts_milliseconds;
use chrono::{DateTime, Utc};
use futures_util::SinkExt;
use log::{debug, trace};
use pin_project::pin_project;
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
            volga_options: Default::default(),
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
    /// Get all unread messages
    #[serde(rename = "unread")]
    Unread,
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

        let cmd = json!({
            "op": "open-consumer",
            "url": self.volga_url.as_str(),
            "name": self.name,
            "position": self.options.position,
            "opts": self.options.volga_options,
        });

        let json = serde_json::to_string_pretty(&cmd)?;
        debug!("{}", json);

        ws.send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;

        super::get_ok_volga_response(&mut ws).await?;

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

/// Metadata on the Volga message received in 'Consumer::consume'
#[derive(Debug, Deserialize)]
pub struct MessageMetadata {
    /// Consumer name
    pub name: String,
    /// True if a control message
    pub control: bool,
    /// Timestamp (seconds since epoch)
    #[serde(with = "ts_milliseconds")]
    pub time: DateTime<Utc>,
    /// Sequence number
    pub seqno: u64,
    /// The number of remaining message the client has indicated it can handle,
    /// see the [Consumer more](struct.Consumer.html) function.
    pub remain: u64,
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
    pub async fn consume(&mut self) -> Result<(MessageMetadata, Vec<u8>)> {
        let timeout = std::time::Duration::from_secs(20);

        loop {
            // First get the control message
            match tokio::time::timeout(timeout, super::get_binary_response(&mut self.ws)).await {
                Err(_) => {
                    trace!("Sending ping");
                    let ping = tokio_tungstenite::tungstenite::Message::Ping(vec![0; 1]);
                    self.ws.send(ping).await?;
                }
                Ok(msg) => {
                    let msg = msg?;
                    let resp: MessageMetadata = serde_json::from_slice(&msg)?;
                    self.last_seq_no = resp.seqno;

                    // Read the actual message
                    let msg = super::get_binary_response(&mut self.ws).await?;

                    if resp.remain == 0 && self.options.auto_more {
                        self.more(N_IN_AUTO_MORE).await?;
                    }
                    return Ok((resp, msg));
                }
            }
        }
    }

    /// Wait for a message for a maximum time. At timeout, None is returned.
    pub async fn consume_with_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<Option<(MessageMetadata, Vec<u8>)>> {
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
//     type Item = Result<(MessageMetadata, Vec<u8>)>;

//     fn poll_next(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Option<Self::Item>> {
//         let mut this = self.project();
//         match core::pin::Pin::new(&mut this.ws).poll_next(cx) {
//             core::task::Poll::Ready(val) => match val {
//                 Some(Ok(val)) => {
//                     // Complicated as we have to wait for two messages, metadata and message
//                     todo!()
//                 }

//                 Some(Err(e)) => core::task::Poll::Ready(Some(Err(e.into()))),
//                 None => core::task::Poll::Ready(None),
//             },
//             core::task::Poll::Pending => core::task::Poll::Pending,
//         }
//     }
// }

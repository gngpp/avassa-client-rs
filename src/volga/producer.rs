use super::{Options, WebSocketStream};
use crate::Result;
use futures_util::SinkExt;
use log::debug;
use serde::Serialize;
use serde_json::json;
use tokio_tungstenite::{client_async, tungstenite::Message as WSMessage};

/// [`Producer`] builder
pub struct ProducerBuilder<'a> {
    avassa_client: &'a crate::Client,
    volga_url: url::Url,
    ws_url: url::Url,
    name: &'a str,
    options: Options,
}

impl<'a> ProducerBuilder<'a> {
    /// Create new Producer builder
    pub fn new(
        avassa_client: &'a crate::Client,
        name: &'a str,
        topic: &str,
        options: Options,
    ) -> Result<Self> {
        let volga_url = url::Url::parse(&format!("volga://localhost/{}", topic,))?;

        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            volga_url,
            ws_url,
            name,
            options,
        })
    }

    pub(crate) fn new_nat(
        avassa_client: &'a crate::Client,
        name: &'a str,
        topic: &str,
        udc: &str,
        options: Options,
    ) -> Result<Self> {
        let volga_url = url::Url::parse(&format!("volga-nat://{}/{}", udc, topic,))?;

        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            volga_url,
            ws_url,
            name,
            options,
        })
    }

    /// Set Volga [`Options`]
    pub fn set_options(self, options: Options) -> Self {
        Self { options, ..self }
    }

    /// Connect and create a [`Producer`]
    pub async fn connect(self) -> Result<Producer> {
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
        super::get_ok_volga_response(&mut ws).await?;
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
    pub async fn produce(&mut self, content: Vec<u8>) -> Result<()> {
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

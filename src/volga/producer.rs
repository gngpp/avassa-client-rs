use super::{Options, WebSocketStream};
use crate::Result;
use futures_util::SinkExt;
use serde::Serialize;
use serde_json::json;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message as WSMessage};

/// [`Producer`] builder
pub struct Builder<'a> {
    avassa_client: &'a crate::Client,
    location: &'a str,
    nat_site: Option<&'a str>,
    topic: &'a str,
    ws_url: url::Url,
    name: &'a str,
    options: Options,
}

impl<'a> Builder<'a> {
    /// Create new Producer builder
    pub fn new(
        avassa_client: &'a crate::Client,
        name: &'a str,
        topic: &'a str,
        options: Options,
    ) -> Result<Self> {
        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            location: "local",
            nat_site: None,
            topic,
            ws_url,
            name,
            options,
        })
    }

    pub(crate) fn new_nat(
        avassa_client: &'a crate::Client,
        name: &'a str,
        topic: &'a str,
        udc: &'a str,
        options: Options,
    ) -> Result<Self> {
        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            location: "nat-site",
            nat_site: Some(udc),
            topic,
            ws_url,
            name,
            options,
        })
    }

    /// Set Volga [`Options`]
    #[must_use]
    pub fn set_options(self, options: Options) -> Self {
        Self { options, ..self }
    }

    /// Connect and create a [`Producer`]
    #[tracing::instrument(level = "debug", skip(self))]
    pub async fn connect(self) -> Result<Producer> {
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

        let cmd = if self.location == "nat-site" {
            json!({
                "op": "open-producer",
                "location": self.location,
                "nat-site": self.nat_site.unwrap(),
                "topic": self.topic,
                "name": self.name,
                "opts": self.options,
            })
        } else {
            json!({
                "op": "open-producer",
                "location": self.location,
                "topic": self.topic,
                "name": self.name,
                "opts": self.options,
            })
        };

        tracing::debug!("{:?}", serde_json::to_string_pretty(&cmd));

        ws.send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;

        tracing::debug!("Waiting for ok");
        super::get_ok_volga_response(&mut ws).await?;
        tracing::debug!("Successfully connected producer to topic {}", self.topic);
        Ok(Producer { ws })
    }
}

/// Volga Producer
pub struct Producer {
    ws: WebSocketStream,
}
impl Producer {
    /// Produce message
    #[tracing::instrument(level = "trace", skip(self))]
    pub async fn produce(&mut self, content: &str) -> Result<()> {
        let cmd = json!({
            "op": "produce",
            "mode": "sync",
            "payload": content,
        });

        tracing::debug!("Cmd: {}", cmd);

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

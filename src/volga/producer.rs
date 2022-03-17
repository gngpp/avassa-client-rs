use super::WebSocketStream;
use crate::Result;
use futures_util::SinkExt;
use serde::Serialize;
use serde_json::json;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message as WSMessage};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
struct OpenProducer<'a> {
    op: &'a str,
    location: super::Location,
    #[serde(skip_serializing_if = "Option::is_none")]
    child_site: Option<&'a str>,
    topic: &'a str,
    name: &'a str,
    #[serde(flatten)]
    on_no_exists: super::OnNoExists,
}

/// [`Producer`] builder
pub struct Builder<'a> {
    avassa_client: &'a crate::Client,
    open_producer: OpenProducer<'a>,
    ws_url: url::Url,
}

impl<'a> Builder<'a> {
    /// Create new Producer builder
    pub fn new(
        avassa_client: &'a crate::Client,
        name: &'a str,
        topic: &'a str,
        on_no_exists: super::OnNoExists,
    ) -> Result<Self> {
        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            ws_url,
            open_producer: OpenProducer {
                op: "open-producer",
                location: super::Location::Local,
                child_site: None,
                topic,
                name,
                on_no_exists,
            },
        })
    }

    pub(crate) fn new_child(
        avassa_client: &'a crate::Client,
        name: &'a str,
        topic: &'a str,
        udc: &'a str,
        on_no_exists: super::OnNoExists,
    ) -> Result<Self> {
        let ws_url = avassa_client.websocket_url.join("volga")?;

        Ok(Self {
            avassa_client,
            ws_url,
            open_producer: OpenProducer {
                op: "open-producer",
                location: super::Location::ChildSite,
                child_site: Some(udc),
                topic,
                name,
                on_no_exists,
            },
        })
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

        tracing::debug!("{}", serde_json::to_string(&self.open_producer)?);

        ws.send(WSMessage::Binary(serde_json::to_vec(&self.open_producer)?))
            .await?;

        tracing::debug!("Waiting for ok");
        super::get_ok_volga_response(&mut ws).await?;
        tracing::debug!(
            "Successfully connected producer to topic {}",
            self.open_producer.topic
        );
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
    pub async fn produce<T: serde::Serialize + std::fmt::Debug>(
        &mut self,
        content: &T,
    ) -> Result<()> {
        let cmd = json!({
            "op": "produce",
            "payload": content,
        });

        tracing::debug!("Cmd: {}", cmd);

        self.ws
            .send(WSMessage::Binary(serde_json::to_vec(&cmd)?))
            .await?;

        tracing::debug!("Waiting for ok");
        // TODO: only if sync
        super::get_ok_volga_response(&mut self.ws).await?;

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

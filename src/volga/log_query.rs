use crate::Result;
use futures_util::{
    stream::{Stream, StreamExt},
    SinkExt,
};
use pin_project::pin_project;
use serde::Serialize;
use tokio_tungstenite::{client_async, tungstenite::Message as WSMessage};

/// Used to query logs 'since'.
#[derive(Clone)]
pub enum Since {
    ///
    Seconds(u64),
    ///
    Minutes(u64),
    ///
    Hours(u64),
}

impl serde::ser::Serialize for Since {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        match self {
            Self::Seconds(v) => serializer.serialize_str(&format!("{}s", v)),
            Self::Minutes(v) => serializer.serialize_str(&format!("{}m", v)),
            Self::Hours(v) => serializer.serialize_str(&format!("{}h", v)),
        }
    }
}

macro_rules! query_setter {
    ($name: ident, $type:ty, $doc:literal) => {
        #[doc=$doc]
        pub fn $name(self, $name: &$type) -> Query {
            Self {
                $name: Some($name.into()),
                ..self
            }
        }
    };
}

/// Log query parameters
#[derive(Serialize)]
pub struct Query {
    op: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    since: Option<Since>,

    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    service: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    ix: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    application: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    search_error: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    re: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    deep_re: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    re_hits: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    dc: Option<String>,
}

impl Query {
    /// Create a query instance
    pub fn new() -> Self {
        Self {
            op: "query_logs".into(),
            since: None,
            name: None,
            service: None,
            ix: None,
            application: None,
            search_error: None,
            re: None,
            deep_re: None,
            re_hits: None,
            count: None,
            dc: None,
        }
    }

    /// The name of the docker image
    pub fn image_name(self, image_name: &str) -> Self {
        Self {
            application: Some(image_name.into()),
            ..self
        }
    }

    /// When we have multiple replicas, by deafult all replicated
    /// logs are read and merged, if we wish to read only one
    /// replica log, we can indicate which replica index to follow
    pub fn replica_index(self, ix: u32) -> Self {
        Self {
            ix: Some(ix),
            ..self
        }
    }

    query_setter!(application, str, "Filter on application name");

    ///Get logs since
    pub fn since(self, since: &Since) -> Self {
        Self {
            since: Some(since.clone()),
            ..self
        }
    }

    query_setter!(service, str, "Filter on service name");

    query_setter!(dc, str, "Filter on datacenter name");

    query_setter!(re, str, "Merge all logs and search the merged result for the provided perl regular expression. Drop all data until a regular expression matches");

    query_setter!(deep_re, str, "Evaluate the regular expression on all nodes where the containers run, for each node, drop all data until regular expression matches.");

    query_setter!(
        count,
        str,
        "Count the number of matching regular expressions"
    );

    /// This is a shorthand to search for the first error in all
    /// logs. Can be combined with [`Self::since`] and [`Self::re_hits`]
    pub fn search_error(self) -> Self {
        Self {
            search_error: Some(true),
            ..self
        }
    }

    /// With either of the regular expression searches, continue
    /// to drop data until `re_hits` log entries have matched
    pub fn re_hits(self, re_hits: u64) -> Self {
        Self {
            re_hits: Some(re_hits),
            ..self
        }
    }
}

/// Stream for query results
#[pin_project]
pub struct QueryStream {
    ws: super::WebSocketStream,
}

impl QueryStream {
    pub(crate) async fn new(avassa_client: &crate::Client, query: &Query) -> Result<Self> {
        let ws_url = avassa_client.websocket_url.join("volga")?;
        let request = tungstenite::handshake::client::Request::builder()
            .uri(ws_url.to_string())
            .header(
                "Authorization",
                format!("Bearer {}", avassa_client.bearer_token().await),
            )
            .body(())
            .map_err(tungstenite::error::Error::HttpFormat)?;
        let tls = avassa_client.open_tls_stream().await?;
        let (mut ws, _) = client_async(request, tls).await?;

        let json = serde_json::to_string_pretty(&query)?;
        log::debug!("{}", json);

        ws.send(WSMessage::Binary(serde_json::to_vec(&query)?))
            .await?;

        Ok(Self { ws })
    }

    /// Try to read one message
    pub async fn recv(&mut self) -> Result<Option<String>> {
        match self.ws.next().await {
            Some(Ok(val)) => match val {
                WSMessage::Binary(v) => Ok(Some(String::from_utf8_lossy(&v).to_string())),
                _ => unreachable!(),
            },
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }
}

impl Stream for QueryStream {
    type Item = crate::Result<String>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Option<Self::Item>> {
        let mut this = self.project();
        // core::pin::Pin::new(&mut this.ws).poll_next(cx)
        match core::pin::Pin::new(&mut this.ws).poll_next(cx) {
            core::task::Poll::Ready(val) => {
                let res: Option<Self::Item> = match val {
                    Some(Ok(WSMessage::Binary(m))) => Some(Ok(String::from_utf8_lossy(&m).into())),
                    Some(Ok(msg)) => Some(Err(crate::Error::Volga(Some(format!(
                        "Unexpected message ({:?})",
                        msg
                    ))))),
                    Some(Err(e)) => Some(Err(e.into())),
                    None => None,
                    // Ok(_) => Err,
                };

                core::task::Poll::Ready(res)
            }
            core::task::Poll::Pending => core::task::Poll::Pending,
        }
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn setter() {
        let query = super::Query::new().application("foo");
        assert_eq!(&query.application.unwrap(), "foo");
    }
}
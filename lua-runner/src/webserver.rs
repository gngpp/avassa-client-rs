use std::collections::HashMap;
use tracing::debug;
use warp::{path::FullPath, Filter};

pub(crate) struct WebServer {
    state: std::sync::Arc<parking_lot::Mutex<State>>,
    shutdown_rx: parking_lot::Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}
struct State {
    paths: HashMap<String, (u16, String)>,
    script_start: std::time::Instant,
    // avassa_client: Option<avassa_client::AvassaClient>,
}

impl WebServer {
    pub fn new() -> (Self, tokio::sync::oneshot::Sender<()>) {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        (
            Self {
                state: std::sync::Arc::new(parking_lot::Mutex::new(State {
                    paths: HashMap::new(),
                    script_start: std::time::Instant::now(),
                })),
                shutdown_rx: parking_lot::Mutex::new(Some(shutdown_rx)),
            },
            shutdown_tx,
        )
    }
    pub async fn start(&self) {
        let state = self.state.clone();
        let routes = warp::path::full().and(warp::filters::query::query()).map(
            move |path: FullPath, q: HashMap<String, String>| {
                debug!("path: {:#?} q: {:#?}", path, q);

                let s = state.lock();
                if path.as_str() == "/ok-then-fail" {
                    let uptime = std::time::Instant::now() - s.script_start;

                    let delay_secs: u64 = q.get("delay").and_then(|d| d.parse().ok()).unwrap_or(20);
                    let ok_after_secs: u64 =
                        q.get("ok_after").and_then(|d| d.parse().ok()).unwrap_or(0);

                    let builder = http::Response::builder();
                    return if uptime > std::time::Duration::from_secs(ok_after_secs)
                        && uptime < std::time::Duration::from_secs(delay_secs)
                    {
                        builder.status(http::StatusCode::OK).body(String::new())
                    } else {
                        builder
                            .status(http::StatusCode::REQUEST_TIMEOUT)
                            .body(String::new())
                    };
                }

                let builder = http::Response::builder();
                match s.paths.get(path.as_str()) {
                    Some((code, content)) => builder
                        .status(http::StatusCode::from_u16(*code).unwrap())
                        .body(content.to_string()),
                    None => builder
                        .status(http::StatusCode::NOT_FOUND)
                        .body(format!("{} not found", path.as_str())),
                }
            },
        );

        let shutdown_rx = self.shutdown_rx.lock().take().unwrap();

        debug!("Starting server");
        let (_addr, server) =
            warp::serve(routes).bind_with_graceful_shutdown(([0, 0, 0, 0], 8080), async {
                shutdown_rx.await.ok();
                debug!("Received stop signal");
            });

        server.await;
    }

    pub fn set_content(&self, path: String, return_code: u16, content: String) {
        let mut state = self.state.lock();
        state.paths.insert(path, (return_code, content));
    }
}

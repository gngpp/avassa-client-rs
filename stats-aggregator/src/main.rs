use hyper::{Body, Request, Response, Server};
use lazy_static::lazy_static;
use log::{error, info};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Deserialize)]
struct TenantStats {
    tenant: String,
    entries: Vec<Stats>,
}

#[derive(Debug, Deserialize)]
struct Stats {
    appname: String,
    cname: String,
    host: String,
    ix: u64,
    srvname: String,
    time: u64,
    #[serde(rename = "memory.usage_in_bytes")]
    mem_usage_bytes: u64,
    #[serde(rename = "cpuacct.usage")]
    cpu_usage: u64,
}

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type HyperResult = std::result::Result<Response<Body>, GenericError>;

lazy_static! {
    static ref CPU_USAGE: prometheus::GaugeVec = prometheus::register_gauge_vec!(
        prometheus::opts!("total_cpu_usage_seconds", "Aggregated CPU Usage in seconds"),
        &["application", "tenant", "datacenter"]
    )
    .unwrap();
    static ref MEM_USAGE: prometheus::IntGaugeVec = prometheus::register_int_gauge_vec!(
        prometheus::opts!("mem_usage_bytes", "Memory Usage in bytes"),
        &["application", "tenant", "datacenter"]
    )
    .unwrap();
}

struct State {}

type StateArc = Arc<Mutex<State>>;

macro_rules! be {
    ($expression:expr) => {
        match $expression {
            Err(_) => break,
            Ok(v) => v,
        }
    };
}

async fn consumer_loop(
    avassa: &avassa_client::Client,
    dc: String,
    _state: StateArc,
) -> anyhow::Result<()> {
    let options = avassa_client::volga::ConsumerOptions {
        volga_options: avassa_client::volga::Options {
            persistence: avassa_client::volga::Persistence::RAM,
            create: true,
            ..Default::default()
        },
        ..Default::default()
    };

    loop {
        info!("Connecting to stats stream for {}", &dc);
        if let Ok(mut consumer) = avassa
            .volga_open_nat_consumer("stats-aggregator", "tenant-stats", &dc, options)
            .await
        {
            info!("Successfully opened consumer in {}", dc);
            loop {
                let (_, msg) = be!(consumer
                    // .consume_with_timeout(std::time::Duration::from_secs(60))
                    .consume()
                    .await);
                let msg: TenantStats = be!(serde_json::from_slice(&msg));
                info!("msg: {:#?}", msg);

                for entry in msg.entries {
                    let labels_vec = vec![
                        ("datacenter", dc.as_str()),
                        ("tenant", msg.tenant.as_str()),
                        ("application", entry.appname.as_str()),
                    ];
                    let labels: HashMap<&str, &str> = labels_vec.iter().cloned().collect();
                    let gauge = MEM_USAGE.with(&labels);
                    gauge.set(entry.mem_usage_bytes as i64);

                    let gauge = CPU_USAGE.with(&labels);
                    gauge.set(std::time::Duration::from_nanos(entry.cpu_usage).as_secs_f64());
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn start_consumers(avassa: avassa_client::Client, dcs: &[String], state: StateArc) {
    for dc in dcs {
        let avassa = avassa.clone();
        let dc = dc.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let _ = consumer_loop(&avassa, dc, state).await;
        });
    }
}

async fn prometheus() -> HyperResult {
    use prometheus::{Encoder, TextEncoder};

    let enc = TextEncoder::new();
    let data = prometheus::gather();
    let mut buf = Vec::new();
    enc.encode(&data, &mut buf)?;

    let response = Response::builder().body(Body::from(buf))?;

    Ok(response)
}

async fn router(_state: StateArc, req: hyper::Request<hyper::Body>) -> HyperResult {
    match (req.method(), req.uri().path()) {
        (_, "/metrics") => prometheus().await,
        (m, p) => {
            error!("{} - {}", m, p);
            unreachable!();
        }
    }
}

async fn run_webserver(state: StateArc) -> anyhow::Result<()> {
    use hyper::service::{make_service_fn, service_fn};

    let make_svc = make_service_fn(move |_| {
        let state = state.clone();
        async move {
            Ok::<_, GenericError>(service_fn(move |req: Request<Body>| {
                router(state.clone(), req)
            }))
        }
    });

    let addr = ([0, 0, 0, 0], 9000).into();

    let server = Server::bind(&addr).serve(make_svc);

    let _ = server.await;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    info!("Build Timestamp: {}", env!("VERGEN_BUILD_TIMESTAMP"));

    let avassa = examples_common::login().await?;

    let dcs = avassa_client::utilities::state::assigned_datacenters(&avassa)
        .await?
        .into_iter()
        .filter(|dc| dc.name != "topdc")
        .map(|dc| dc.name)
        .collect::<Vec<_>>();

    let state = Arc::new(Mutex::new(State {}));

    start_consumers(avassa.clone(), &dcs, state.clone()).await;

    run_webserver(state).await
}

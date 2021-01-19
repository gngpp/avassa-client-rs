use hyper::{Body, Request, Response, Server};
use lazy_static::lazy_static;
use std::collections::HashMap;
use tracing::{error, info};

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type HyperResult = std::result::Result<Response<Body>, GenericError>;

lazy_static! {
    static ref TEMPERATURE_GV: prometheus::GaugeVec = prometheus::register_gauge_vec!(
        prometheus::opts!("temperatures", "Returning building temperatures"),
        &["building"]
    )
    .unwrap();
    static ref FUNCTIONAL_TEMP_SENSORS: prometheus::IntGaugeVec =
        prometheus::register_int_gauge_vec!(
            prometheus::opts!(
                "functional_temp_sensors",
                "Number of temp sensors that work"
            ),
            &["building"]
        )
        .unwrap();
    static ref TOTAL_TEMP_SENSORS: prometheus::IntGaugeVec = prometheus::register_int_gauge_vec!(
        prometheus::opts!("total_temp_sensors", "Total Number of temp sensors"),
        &["building"]
    )
    .unwrap();
    static ref VENT_POWER: prometheus::GaugeVec = prometheus::register_gauge_vec!(
        prometheus::opts!(
            "ventilation_power_consumption",
            "Current ventilation power consumption (kW)"
        ),
        &["building"]
    )
    .unwrap();
}

async fn login() -> anyhow::Result<avassa_client::Client> {
    let supd = std::env::var("SUPD").expect("Failed to get SUPD");
    let avassa = match avassa_client::Client::application_login(&supd).await {
        Ok(client) => Ok(client),
        Err(_) => avassa_client::Client::login(&supd, "joe@acme.com", "verysecret").await,
    }?;
    Ok(avassa)
}

macro_rules! be {
    ($expression:expr) => {
        match $expression {
            Err(_) => break,
            Ok(v) => v,
        }
    };
}

async fn consumer_loop(avassa: &avassa_client::Client, dc: String) -> anyhow::Result<()> {
    let options = avassa_client::volga::Options {
        persistence: avassa_client::volga::Persistence::RAM,
        create: true,
        ..Default::default()
    };

    loop {
        info!("Connecting consumer for {}", &dc);
        if let Ok(mut consumer) = avassa
            .volga_open_nat_consumer("aggregator", "temperatures", &dc, options)
            .await
        {
            loop {
                let msg = be!(consumer.consume().await);
                let msg: hvac_common::Message = be!(serde_json::from_slice(&msg));
                info!("msg: {:#?}", msg);

                match msg {
                    hvac_common::Message::TempReport(msg) => {
                        let labels: HashMap<&str, &str> =
                            vec![("building", msg.name.as_str())].into_iter().collect();
                        let gauge = TEMPERATURE_GV.with(&labels);
                        gauge.set(msg.temperature as f64);
                    }
                    hvac_common::Message::BuildingReport(report) => {
                        let labels: HashMap<&str, &str> = vec![("building", report.name.as_str())]
                            .into_iter()
                            .collect();

                        FUNCTIONAL_TEMP_SENSORS
                            .with(&labels)
                            .set(report.functional_temp_sensors as i64);
                        TOTAL_TEMP_SENSORS
                            .with(&labels)
                            .set(report.total_temp_sensors as i64);
                        VENT_POWER
                            .with(&labels)
                            .set(report.ventilation_power_consumption as f64);
                    }
                }
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn start_consumers(avassa: avassa_client::Client, dcs: &[String]) {
    for dc in dcs {
        let avassa = avassa.clone();
        let dc = dc.clone();
        tokio::spawn(async move {
            let _ = consumer_loop(&avassa, dc).await;
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

async fn router(req: hyper::Request<hyper::Body>) -> HyperResult {
    match (req.method(), req.uri().path()) {
        (_, "/metrics") => prometheus().await,
        (m, p) => {
            error!("{} - {}", m, p);
            unreachable!();
        }
    }
}

async fn run_webserver() -> anyhow::Result<()> {
    use hyper::service::{make_service_fn, service_fn};

    let make_svc = make_service_fn(move |_| async move {
        Ok::<_, GenericError>(service_fn(move |req: Request<Body>| router(req)))
    });

    let addr = ([0, 0, 0, 0], 9000).into();

    let server = Server::bind(&addr).serve(make_svc);

    let _ = server.await;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // tracing_subscriber::fmt::init();
    info!("Build Timestamp: {}", env!("VERGEN_BUILD_TIMESTAMP"));

    let avassa = login().await?;

    let dcs: Vec<String> = avassa
        .get_json::<serde_json::Value>("/v1/config/tenants/acme/datacenters", None)
        .await?
        .as_array()
        .expect("Failed to get DC list")
        .into_iter()
        .map(|d| {
            d["name"]
                .as_str()
                .expect(&format!("Failed to get name of {:?}", d))
                .to_string()
        })
        .filter(|n| n != "topdc")
        .collect();

    start_consumers(avassa.clone(), &dcs).await;

    run_webserver().await
}

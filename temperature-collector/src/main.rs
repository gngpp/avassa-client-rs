use anyhow::Context;
use log::info;
use rand::distributions::{Distribution, Uniform};

const POWER_CONS_PER_FLOOR: f32 = 15.;

fn building_report(name: &str) -> hvac_common::BuildingReport {
    let mut rng = rand::thread_rng();

    let floors = Uniform::new(3, 8).sample(&mut rng);
    let total_temp_sensors = Uniform::new(floors, 3 * floors).sample(&mut rng);

    hvac_common::BuildingReport {
        name: name.to_string(),
        floors,
        total_temp_sensors,
        functional_temp_sensors: total_temp_sensors,
        ventilation_power_consumption: (floors as f32) * POWER_CONS_PER_FLOOR * 0.3,
        cooling_active_last_hour: 0.0,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    info!("Build Timestamp: {}", env!("VERGEN_BUILD_TIMESTAMP"));

    let supd = std::env::var("SUPD").expect("Failed to get SUPD");
    let avassa = match avassa_client::Client::application_login(&supd).await {
        Ok(client) => Ok(client),
        Err(_) => avassa_client::Client::login(&supd, "joe@acme.com", "verysecret").await,
    }?;

    // Find out the name of the DC
    let supd_dcs = avassa
        .get_json::<serde_json::Value>("/v1/state/cluster", None)
        .await
        .context("get cluster info")?;
    let dc = supd_dcs
        // Convert to an object
        .as_object()
        .expect("Failed to parse dc data");
    let name = dc["cluster_id"].as_str().expect("Failed to read name");
    info!("DC name: {}", name);

    let volga_opts = avassa_client::volga::Options {
        persistence: avassa_client::volga::Persistence::RAM,
        ..Default::default()
    };

    let mut producer = avassa
        .volga_open_producer("collector", "temperatures", volga_opts)
        .await?;

    let mut rng = rand::thread_rng();

    let temp_change = Uniform::new(-0.2, 0.2);

    let mut temperature = Uniform::new(15.0, 25.0).sample(&mut rng);

    let mut building_report = building_report(name);

    let percent: Uniform<f32> = Uniform::new(0., 1.);
    let max_vent_consumption = building_report.floors as f32 * POWER_CONS_PER_FLOOR;

    let mut cur_floor_cons = percent.sample(&mut rng);

    loop {
        let timestamp = chrono::Utc::now();

        let report = hvac_common::Message::TempReport(hvac_common::TempReport {
            name: name.to_string(),
            timestamp,
            temperature,
        });

        info!("Reporting {:#?}", report);

        producer.produce(serde_json::to_string(&report)?).await?;

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        temperature += temp_change.sample(&mut rng);

        building_report.functional_temp_sensors = 0;
        for _ in 0..building_report.total_temp_sensors {
            if percent.sample(&mut rng) > 0.1 {
                building_report.functional_temp_sensors += 1;
            }
        }
        building_report.ventilation_power_consumption = cur_floor_cons * max_vent_consumption;
        loop {
            cur_floor_cons += Uniform::new(-0.05, 0.05).sample(&mut rng);
            info!("cur_floor_cons: {}", cur_floor_cons);
            if cur_floor_cons > 0. && cur_floor_cons < 1. {
                break;
            }
        }

        producer
            .produce(serde_json::to_string(
                &hvac_common::Message::BuildingReport(building_report.clone()),
            )?)
            .await?;
    }
}

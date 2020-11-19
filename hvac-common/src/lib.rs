use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TempReport {
    pub name: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub temperature: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildingReport {
    pub name: String,
    pub floors: u8,
    pub functional_temp_sensors: u8,
    pub total_temp_sensors: u8,
    pub ventilation_power_consumption: f32,
    pub cooling_active_last_hour: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Message {
    TempReport(TempReport),
    BuildingReport(BuildingReport),
}

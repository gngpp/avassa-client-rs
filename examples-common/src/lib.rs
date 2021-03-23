use anyhow::Context;
use log::info;

// #[derive(Debug, thiserror::Error)]
// enum Error {
//     #[error("Failed to parse JSON: {0}")]
//     JSON(String),
// }

pub async fn login() -> anyhow::Result<avassa_client::Client> {
    let supd = std::env::var("SUPD").context("SUPD")?;
    info!("Connecting to api {}", supd);
    let avassa = match avassa_client::Client::application_login(&supd).await {
        Ok(client) => Ok(client),
        Err(e) => {
            let username = std::env::var("SUPD_USER").context("SUPD_USER")?;
            let password = std::env::var("SUPD_PASSWORD").context("SUPD_USER")?;
            info!("App role login failed ({}), trying username/password", e);
            avassa_client::Client::login(&supd, &username, &password).await
        }
    }?;
    info!("Successfully logged in");
    Ok(avassa)
}

// fn flt<T, E>(items: Vec<std::result::Result<T, E>>) -> std::result::Result<Vec<T>, E> {
//     let mut res = Vec::with_capacity(items.len());
//     for i in items.into_iter() {
//         res.push(i?);
//     }
//     Ok(res)
// }

// pub async fn datacenter_names(avassa: &avassa_client::Client) -> anyhow::Result<Vec<String>> {
//     let dc_names: Vec<Result<String, Error>> = avassa
//         .get_json::<serde_json::Value>("/v1/state/assigned_datacenters", None)
//         .await?
//         .as_array()
//         .ok_or(Error::JSON("Failed to get dc array".into()))?
//         .into_iter()
//         .map(|d| {
//             d["name"]
//                 .as_str()
//                 .map(|s| s.to_string())
//                 .ok_or(Error::JSON("Failed to get dc name".into()))
//         })
//         .collect();
//     Ok(flt(dc_names)?)
// }

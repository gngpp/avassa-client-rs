use anyhow::{Context, Result};
use log::debug;
use rlua::Lua;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod lua;
mod webserver;

async fn load_script() -> Result<String> {
    if let Ok(script) = std::env::var("SCRIPT") {
        return Ok(script);
    } else if let Ok(script_path) = std::env::var("SCRIPT_PATH") {
        let url = url::Url::parse(&script_path)?;
        match url.scheme() {
            "file" => {
                return Ok(std::fs::read_to_string(std::path::Path::new(url.path()))
                    .context(script_path)?);
            }
            "http" | "https" => {
                let resp = reqwest::get(url).await?;
                if !resp.status().is_success() {
                    return Err(anyhow::Error::msg("HTTP")
                        .context(script_path)
                        .context(resp.status()));
                }
                let script = resp.text().await?;
                return Ok(script);
            }
            scheme => {
                return Err(anyhow::Error::msg(format!(
                    "{} is not a valid source scheme",
                    scheme
                )));
            }
        }
    }

    Err(anyhow::Error::msg(
        "Please set SCRIPT or SCRIPT_PATH env vars",
    ))
}

// #[tokio::main]
fn main() -> Result<()> {
    pretty_env_logger::init_timed();
    println!("Built: {}", env!("VERGEN_BUILD_TIMESTAMP"));

    let lua = Lua::new();

    let results = Arc::new(Mutex::new(HashMap::new()));

    let mut tokio_rt = tokio::runtime::Runtime::new()?;

    let (webserver, stop_ws) = webserver::WebServer::new();
    let webserver = std::sync::Arc::new(webserver);

    let runner_lua = lua::RunnerLua::new(
        webserver.clone(),
        tokio_rt.handle().clone(),
        results.clone(),
    );

    let script = tokio_rt.block_on(load_script())?;

    let ws = webserver.clone();
    tokio_rt.spawn(async move {
        ws.start().await;
    });

    let res = lua.context(|lua_ctx| -> Result<()> {
        lua_ctx.globals().set("avassa", runner_lua)?;

        lua_ctx.load(&script).exec()?;

        Ok(())
    });

    let results = results.lock().expect("Failed to get results");
    for (k, v) in results.iter() {
        println!("{}: {}", k, v);
    }

    debug!("Stopping web server");
    let _ = stop_ws.send(());

    res
}

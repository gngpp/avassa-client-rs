use rlua::{UserData, UserDataMethods};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{error, info, instrument};

mod volga;

type LUAError<T> = (Option<T>, Option<String>);
type LUAResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub fn to_lua_error(e: avassa_client::Error) -> rlua::Error {
    rlua::Error::ExternalError(std::sync::Arc::new(e))
}

trait FromLuaResult<T> {
    fn from_lr(lr: LUAResult<T>) -> Self;
}

impl<T> FromLuaResult<T> for LUAError<T> {
    fn from_lr(lr: LUAResult<T>) -> Self {
        match lr {
            Ok(res) => (Some(res), None),
            Err(e) => (None, Some(format!("{}", e))),
        }
    }
}

#[instrument]
fn ping(host: String, count: u32) -> LUAResult<u32> {
    use fastping_rs::PingResult::{Idle, Receive};
    use fastping_rs::Pinger;
    info!("ping {} x {}", count, host);

    let (pinger, results) = Pinger::new(None, None).map_err(|e| {
        error!("err: {}", e);
        e
    })?;

    pinger.add_ipaddr(&host);
    pinger.run_pinger();

    let mut cnt = 0;

    for _ in 0..count {
        match results.recv()? {
            Idle { addr } => {
                info!("Missing ping to {}", addr);
            }
            Receive { addr, rtt } => {
                info!("pong: {} - {:#?}", addr, rtt);
                cnt += 1;
            }
        }
    }

    pinger.stop_pinger();

    Ok(cnt)
}

struct GetResult {
    status_code: u16,
    text: String,
}

#[tracing::instrument(level = "debug", skip(ctx))]
fn http_get<'lua>(ctx: rlua::Context<'lua>, url: &str) -> LUAResult<rlua::Table<'lua>> {
    let (table_tx, table_rx) = std::sync::mpsc::channel::<
        std::result::Result<GetResult, Box<dyn std::error::Error + Send>>,
    >();

    let table = ctx.create_table().expect("Failed to create table");
    let url = url.to_string();
    info!("Spawning get");
    let handle = tokio::runtime::Handle::current();
    handle.spawn(async move {
        let get = reqwest::get(&url).await;
        if let Err(e) = get {
            let _ = table_tx.send(Err(Box::new(e)));
            return Ok::<(), reqwest::Error>(());
        }

        let get = get.unwrap();
        let status_code = get.status().as_u16();
        info!("Get done");
        let text = get.text().await;

        if let Err(e) = text {
            let _ = table_tx.send(Err(Box::new(e)));
            return Ok(());
        }

        let text = text.unwrap();

        let _ = table_tx.send(Ok(GetResult { status_code, text }));

        Ok(())
    });

    let get_res = table_rx.recv()?;
    let get_res = get_res.unwrap();

    table.set("status_code", get_res.status_code)?;
    table.set("text", get_res.text)?;

    Ok(table)
}

pub(crate) struct RunnerLua {
    rt_handle: tokio::runtime::Handle,
    pub results: Arc<Mutex<HashMap<String, String>>>,
    ws: std::sync::Arc<crate::webserver::WebServer>,
}

impl RunnerLua {
    pub fn new(
        ws: std::sync::Arc<crate::webserver::WebServer>,
        rt_handle: tokio::runtime::Handle,
        results: Arc<Mutex<HashMap<String, String>>>,
    ) -> Self {
        RunnerLua {
            ws,
            rt_handle,
            results,
        }
    }

    async fn login(
        &mut self,
        base_url: String,
        username: String,
        password: String,
    ) -> rlua::Result<Client> {
        let client = avassa_client::Client::login(&base_url, &username, &password).await;

        client
            .map(|client| Client::new(self.rt_handle.clone(), client))
            .map_err(to_lua_error)
    }

    async fn application_login(&mut self, base_url: String) -> rlua::Result<Client> {
        let client = avassa_client::Client::application_login(&base_url).await;

        client
            .map(|client| Client::new(self.rt_handle.clone(), client))
            .map_err(to_lua_error)
    }
}

struct Client {
    rt_handle: tokio::runtime::Handle,
    client: std::sync::Arc<avassa_client::Client>,
}

impl Client {
    fn new(rt_handle: tokio::runtime::Handle, client: avassa_client::Client) -> Self {
        Self {
            rt_handle,
            client: std::sync::Arc::new(client),
        }
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn volga_open_producer(
        &mut self,
        producer_name: String,
        topic: String,
    ) -> rlua::Result<volga::Producer> {
        let prod = self
            .client
            .volga_open_producer(&producer_name, &topic, Default::default())
            .await;

        prod.map(|producer| volga::Producer::new(self.rt_handle.clone(), producer))
            .map_err(to_lua_error)
    }

    #[tracing::instrument(level = "debug", skip(self))]
    async fn volga_open_consumer(
        &mut self,
        consumer_name: String,
        topic: String,
    ) -> rlua::Result<volga::Consumer> {
        let builder = self
            .client
            .volga_open_consumer(&consumer_name, &topic)
            .map_err(to_lua_error)?;

        let consumer = builder.connect().await.map_err(to_lua_error)?;

        Ok(volga::Consumer::new(self.rt_handle.clone(), consumer))
    }
}

impl UserData for Client {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method_mut(
            "volga_open_producer",
            |_, this, args: (String, String)| -> rlua::Result<volga::Producer> {
                let rt_handle = this.rt_handle.clone();
                rt_handle.block_on(this.volga_open_producer(args.0, args.1))
            },
        );

        methods.add_method_mut(
            "volga_open_consumer",
            |_, this, args: (String, String)| -> rlua::Result<volga::Consumer> {
                let rt_handle = this.rt_handle.clone();
                rt_handle.block_on(this.volga_open_consumer(args.0, args.1))
            },
        );

        methods.add_method(
            "get_json",
            |_, this, args: String| -> rlua::Result<String> {
                let rt_handle = this.rt_handle.clone();
                rt_handle.block_on(async move {
                    let json = this
                        .client
                        .get_json(&args, None)
                        .await
                        .map_err(to_lua_error)?;
                    let json = serde_json::to_string_pretty(&json)
                        .map_err(|e| rlua::Error::ExternalError(Arc::new(e)))?;
                    Ok(json)
                })
            },
        );
    }
}

impl UserData for RunnerLua {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("ping", |_, _this, args: (String, u32)| {
            Ok(LUAError::from_lr(ping(args.0, args.1)))
        });

        methods.add_method("http_get", |ctx, _, args: String| {
            Ok(LUAError::from_lr(http_get(ctx, &args)))
        });

        methods.add_method("set_result", |_, this, args: (String, String)| {
            let mut results = this.results.lock().expect("Failed to lock results");
            results.insert(args.0, args.1);
            Ok(())
        });

        methods.add_method("sleep", |_, _, t: u64| {
            std::thread::sleep(std::time::Duration::from_millis(t));
            Ok(())
        });

        // methods.add_method("high_cpu", |_, this, args: usize| {
        //     let rt_handle = this.rt_handle.clone();
        //     rt_handle.block_on(async move {
        //         let cpu_hog = tokio::spawn(async {
        //             todo!();
        //         });
        //         tokio::time::delay_for(std::time::Duration::from_secs(args)).await;
        //         cpu_hog_tx.send(());
        //     });
        //     Ok(())
        // });

        methods.add_method("web_set_content", |_, this, args: (String, u16, String)| {
            this.ws.set_content(args.0, args.1, args.2);
            Ok(())
        });

        methods.add_method_mut(
            "login",
            |_, this, args: (String, String, String, String)| {
                let rt_handle = this.rt_handle.clone();
                rt_handle.block_on(this.login(args.0, args.1, args.2))
            },
        );

        methods.add_method_mut("application_login", |_, this, args: String| {
            let rt_handle = this.rt_handle.clone();
            rt_handle.block_on(this.application_login(args))
        });
    }
}

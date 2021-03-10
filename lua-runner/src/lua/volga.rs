use rlua::{UserData, UserDataMethods};
use std::sync::Arc;

pub(crate) struct Consumer {
    tokio_rt: Arc<tokio::runtime::Runtime>,
    consumer: avassa_client::volga::Consumer,
}

impl Consumer {
    pub(crate) fn new(
        tokio_rt: Arc<tokio::runtime::Runtime>,
        consumer: avassa_client::volga::Consumer,
    ) -> Self {
        Self { tokio_rt, consumer }
    }
}

impl UserData for Consumer {
    fn add_methods<'lua, T: UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method_mut("consume", |_, this, _: ()| {
            let tokio_rt = this.tokio_rt.clone();

            tokio_rt
                .block_on(async move {
                    this.consumer
                        .consume()
                        .await
                        .map_err(|e| rlua::Error::ExternalError(std::sync::Arc::new(e)))
                })
                .map(|(_, v)| String::from_utf8_lossy(&v).to_string())
        });
    }
}
pub(crate) struct Producer {
    tokio_rt: Arc<tokio::runtime::Runtime>,
    producer: avassa_client::volga::Producer,
}

impl Producer {
    pub(crate) fn new(
        tokio_rt: Arc<tokio::runtime::Runtime>,
        producer: avassa_client::volga::Producer,
    ) -> Self {
        Self { tokio_rt, producer }
    }
}

impl UserData for Producer {
    fn add_methods<'lua, T: UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method_mut("produce", |_, this, args: String| {
            let tokio_rt = this.tokio_rt.clone();

            tokio_rt.block_on(async move {
                this.producer
                    .produce(args)
                    .await
                    .map_err(|e| rlua::Error::ExternalError(std::sync::Arc::new(e)))
            })
        });
    }
}

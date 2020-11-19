use rlua::{UserData, UserDataMethods};

pub(crate) struct Consumer {
    rt_handle: tokio::runtime::Handle,
    consumer: avassa_client::volga::Consumer,
}

impl Consumer {
    pub(crate) fn new(
        rt_handle: tokio::runtime::Handle,
        consumer: avassa_client::volga::Consumer,
    ) -> Self {
        Self {
            rt_handle,
            consumer,
        }
    }
}

impl UserData for Consumer {
    fn add_methods<'lua, T: UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method_mut("consume", |_, this, _: ()| {
            let rt_handle = this.rt_handle.clone();

            rt_handle
                .block_on(async move {
                    this.consumer
                        .consume()
                        .await
                        .map_err(|e| rlua::Error::ExternalError(std::sync::Arc::new(e)))
                })
                .map(|v| String::from_utf8_lossy(&v).to_string())
        });
    }
}
pub(crate) struct Producer {
    rt_handle: tokio::runtime::Handle,
    producer: avassa_client::volga::Producer,
}

impl Producer {
    pub(crate) fn new(
        rt_handle: tokio::runtime::Handle,
        producer: avassa_client::volga::Producer,
    ) -> Self {
        Self {
            rt_handle,
            producer,
        }
    }
}

impl UserData for Producer {
    fn add_methods<'lua, T: UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method_mut("produce", |_, this, args: String| {
            let rt_handle = this.rt_handle.clone();

            rt_handle.block_on(async move {
                this.producer
                    .produce(args)
                    .await
                    .map_err(|e| rlua::Error::ExternalError(std::sync::Arc::new(e)))
            })
        });
    }
}

pub(crate) struct InfraProducer {
    rt_handle: tokio::runtime::Handle,
    producer: avassa_client::volga::InfraProducer,
}

impl InfraProducer {
    pub(crate) fn new(
        rt_handle: tokio::runtime::Handle,
        producer: avassa_client::volga::InfraProducer,
    ) -> Self {
        Self {
            rt_handle,
            producer,
        }
    }
}

impl UserData for InfraProducer {
    fn add_methods<'lua, T: UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method_mut("produce", |_, this, args: String| {
            let rt_handle = this.rt_handle.clone();

            rt_handle.block_on(async move {
                this.producer
                    .produce(args)
                    .await
                    .map_err(|e| rlua::Error::ExternalError(std::sync::Arc::new(e)))
            })
        });
    }
}

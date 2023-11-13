fn help() {
    const HELP: &str = r#"
        cargo run --example volga-consume -- https://api my-user my-pwd volga-topic
    "#;
    println!("{HELP}");
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let mut args = std::env::args();
    // drop exec
    let _ = args.next();
    let Some(supd) = args.next() else {
        help();
        return Ok(());
    };
    let Some(username) = args.next() else {
        help();
        return Ok(());
    };
    let Some(password) = args.next() else {
        help();
        return Ok(());
    };
    let Some(topic_name) = args.next() else {
        help();
        return Ok(());
    };
    let builder = avassa_client::Client::builder()
        // Just for testing
        .danger_disable_cert_verification();

    let client = builder.login(&supd, &username, &password).await?;

    let mut consumer = client
        .volga_open_consumer(
            "volga-consumer",
            &topic_name,
            avassa_client::volga::consumer::Options {
                position: avassa_client::volga::consumer::Position::Beginning,
                on_no_exists: avassa_client::volga::OnNoExists::Fail,
                ..Default::default()
            },
        )
        .await?;

    while let Ok(msg) = consumer.consume::<String>().await {
        println!("{}", msg.payload);
    }

    Ok(())
}

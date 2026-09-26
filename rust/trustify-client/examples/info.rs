use trustify_client::TrustifyClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base_url =
        std::env::var("TRUSTIFY_URL").unwrap_or_else(|_| "http://localhost:8080".to_owned());
    let client = TrustifyClient::builder(base_url).build()?;

    let info = client.api().info().send().await?;
    println!("{:#?}", info.into_inner());
    Ok(())
}

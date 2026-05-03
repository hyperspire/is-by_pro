use web_push::*;

#[tokio::main]
async fn main() {
    let vapid_public = "BFK9Nx4vy-L1KpqCfYgMLdw03gDVcbzT3BWuX9Zvd1EoHqwAo7wlxdx9x6nPN9TGL7aUYm_geOlszXHxgOWf3Hs".trim();
    let vapid_private = "o2ofH9CSnF7OLqxZCwWBv3T90PoKX31YKlZKJ9-zpx8".trim();

    let client = IsahcWebPushClient::new().unwrap();
    let subscription_info = SubscriptionInfo::new(
        "https://fcm.googleapis.com/fcm/send/fake_endpoint".to_string(),
        "BMkY5".to_string(),
        "xyz".to_string()
    );

    let mut sig_builder = match VapidSignatureBuilder::from_base64(
        vapid_private,
        &subscription_info,
    ) {
        Ok(builder) => builder,
        Err(e) => {
            eprintln!("VAPID signature builder error: {:?}", e);
            return;
        }
    };

    println!("Signature builder created successfully!");
}

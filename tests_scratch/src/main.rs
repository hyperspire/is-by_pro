use web_push::*;
use serde_json::json;

#[tokio::main]
async fn main() {
    let endpoint = "https://fcm.googleapis.com/fcm/send/dYOQs7Swmmg:APA91bEkT8uJwii4ao1JNH1-WoTisRL0I_3OVoGi5-BQWdaDcypYQknR3Y_GJ44xdDGuGucidB1JzeJGoFZNRkgYlkXlmQHVUs_RxqAAZe0lKv95jag2rs7DhRVITCBg9yZnojecOx-g";
    let p256dh = "BBW6yrjx8bzVBrjrtPVVstJyBjn8SdSdXGXLTRC5Dlubav21i3HXcDVMuuuSEz7DERX0zoXUmlLD4mjmpA4Olfo";
    let auth = "ricdLCXgvFaORpmgJv9TsA";
    
    let vapid_private = "o2ofH9CSnF7OLqxZCwWBv3T90PoKX31YKlZKJ9-zpx8";

    let subscription_info = SubscriptionInfo::new(
        endpoint,
        p256dh,
        auth,
    );

    let mut sig_builder = VapidSignatureBuilder::from_base64(&vapid_private, &subscription_info).unwrap();
    sig_builder.add_claim("sub", json!("mailto:admin@is-by.pro"));
    let signature = sig_builder.build().unwrap();

    let payload = json!({
        "title": "System Test Notification",
        "body": "This is a direct test of the Web Push configuration.",
        "url": "https://is-by.pro/v1/mobile/home"
    }).to_string();

    let mut builder = WebPushMessageBuilder::new(&subscription_info);
    builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_bytes());
    builder.set_vapid_signature(signature);

    let message = builder.build().unwrap();

    let client = IsahcWebPushClient::new().unwrap();
    if let Err(e) = client.send(message).await {
        eprintln!("Failed to send web push: {:?}", e);
    } else {
        println!("Successfully pushed real JSON payload to browser!");
    }
}

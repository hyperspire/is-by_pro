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

    println!("Creating VapidSignatureBuilder...");
    let mut sig_builder = match VapidSignatureBuilder::from_base64(&vapid_private, &subscription_info) {
        Ok(builder) => builder,
        Err(e) => {
            eprintln!("Failed to create VapidSignatureBuilder: {:?}", e);
            return;
        }
    };
    
    sig_builder.add_claim("sub", json!(format!("mailto:admin@{}", "is-by.pro")));
    
    println!("Building signature...");
    let signature = match sig_builder.build() {
        Ok(sig) => sig,
        Err(e) => {
            eprintln!("Failed to build VAPID signature: {:?}", e);
            return;
        }
    };

    println!("Building message...");
    let payload = "hello";
    let mut builder = WebPushMessageBuilder::new(&subscription_info);
    builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_bytes());
    builder.set_vapid_signature(signature);

    let message = match builder.build() {
        Ok(msg) => msg,
        Err(e) => {
            eprintln!("Failed to build web push message: {:?}", e);
            return;
        }
    };

    println!("Sending message...");
    let client = IsahcWebPushClient::new().unwrap();
    if let Err(e) = client.send(message).await {
        eprintln!("Failed to send web push: {:?}", e);
    } else {
        println!("Success!");
    }
}

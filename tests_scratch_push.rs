use sqlx::mysql::MySqlPoolOptions;
use std::env;
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let vapid_env = fs::read_to_string(".env/VAPID.env")?;
    let mut public_key = String::new();
    let mut private_key = String::new();
    for line in vapid_env.lines() {
        if line.starts_with("VAPID_PUBLIC_KEY=") {
            public_key = line.replace("VAPID_PUBLIC_KEY=", "").replace("\"", "").trim().to_string();
        } else if line.starts_with("VAPID_PRIVATE_KEY=") {
            private_key = line.replace("VAPID_PRIVATE_KEY=", "").replace("\"", "").trim().to_string();
        }
    }
    println!("Public Key: {}", public_key);
    println!("Private Key: {}", private_key);

    let mysql_env = fs::read_to_string(".env/MYSQL.env")?;
    let password = mysql_env.lines().find(|l| l.starts_with("MYSQL_PASSWORD=")).unwrap().replace("MYSQL_PASSWORD=", "").replace("\"", "");
    let db_url = format!("mysql://is-by:{}@131.186.5.182:3306/pro", password);

    let pool = MySqlPoolOptions::new().connect(&db_url).await?;

    let rows = sqlx::query!("SELECT endpoint, p256dh, auth FROM push_subscriptions").fetch_all(&pool).await?;
    println!("Found {} subscriptions", rows.len());

    let client = web_push::IsahcWebPushClient::new()?;

    for row in rows {
        println!("Endpoint: {}", row.endpoint);
        let subscription_info = web_push::SubscriptionInfo::new(
            row.endpoint,
            row.p256dh,
            row.auth
        );

        let mut sig_builder = web_push::VapidSignatureBuilder::from_base64(&private_key, &subscription_info)?;
        sig_builder.add_claim("sub", "mailto:admin@is-by.pro");
        let vapid_sig = sig_builder.build()?;

        let mut builder = web_push::WebPushMessageBuilder::new(&subscription_info);
        let payload = serde_json::json!({"title": "Test", "body": "Testing Web Push"}).to_string();
        builder.set_payload(web_push::ContentEncoding::Aes128Gcm, payload.as_bytes());
        builder.set_vapid_signature(vapid_sig);
        builder.set_ttl(86400);

        let message = builder.build()?;
        match client.send(message).await {
            Ok(_) => println!("Push sent successfully!"),
            Err(e) => println!("Push failed: {:?}", e),
        }
    }

    Ok(())
}

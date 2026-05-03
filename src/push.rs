use web_push::*;
use crate::models::{AppState, PushSubscriptionRow};

pub async fn send_push_notification(
    state: &AppState,
    target_uid: i64,
    title: &str,
    body: &str,
    url: &str,
) {
    let subscriptions = match sqlx::query_as::<_, PushSubscriptionRow>(
        "SELECT endpoint, p256dh, auth FROM push_subscriptions WHERE ib_uid = ?"
    )
    .bind(target_uid)
    .fetch_all(&state.db_pool)
    .await
    {
        Ok(subs) => subs,
        Err(e) => {
            eprintln!("Failed to fetch push subscriptions: {}", e);
            return;
        }
    };

    if subscriptions.is_empty() {
        return;
    }

    let client = match IsahcWebPushClient::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create web push client: {:?}", e);
            return;
        }
    };

    let payload_str = serde_json::json!({
        "title": title,
        "body": body,
        "url": url
    }).to_string();

    let payload_bytes = payload_str.as_bytes();

    for sub in subscriptions {
        let subscription_info = SubscriptionInfo::new(
            sub.endpoint.clone(),
            sub.p256dh.clone(),
            sub.auth.clone()
        );

        let mut sig_builder = match VapidSignatureBuilder::from_base64(
            state.vapid_private_key.trim(),
            &subscription_info,
        ) {
            Ok(builder) => builder,
            Err(e) => {
                eprintln!("VAPID signature builder error: {:?}", e);
                continue;
            }
        };

        sig_builder.add_claim("sub", "mailto:admin@is-by.pro");

        let vapid_sig = match sig_builder.build() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("VAPID build error: {:?}", e);
                continue;
            }
        };

        let mut builder = WebPushMessageBuilder::new(&subscription_info);
        builder.set_payload(web_push::ContentEncoding::Aes128Gcm, payload_bytes);
        builder.set_vapid_signature(vapid_sig);
        builder.set_ttl(86400);

        match builder.build() {
            Ok(message) => {
                if let Err(e) = client.send(message).await {
                    eprintln!("Push send error: {:?}", e);
                }
            }
            Err(e) => eprintln!("Push message build error: {:?}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_web_push_builder() {
        let vapid_private = "o2ofH9CSnF7OLqxZCwWBv3T90PoKX31YKlZKJ9-zpx8";
        let subscription_info = SubscriptionInfo::new(
            "https://fcm.googleapis.com/fcm/send/fake".to_string(),
            "BMkY5".to_string(),
            "xyz".to_string()
        );

        let sig_builder = VapidSignatureBuilder::from_base64(
            vapid_private,
            &subscription_info,
        );

        match sig_builder {
            Ok(_) => println!("Builder OK!"),
            Err(e) => println!("Builder Error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_real_push() {
        let _ = dotenvy::from_path(".env/MYSQL.env");
        let _ = dotenvy::from_path(".env/VAPID.env");
        
        let vapid_private = std::env::var("VAPID_PRIVATE_KEY").unwrap_or_default();
        let mysql_password = std::env::var("MYSQL_PASSWORD").unwrap_or_default();
        let mysql_host = std::env::var("MYSQL_HOST").unwrap_or_default();
        let mysql_user = std::env::var("MYSQL_USER").unwrap_or_default();
        let mysql_db = std::env::var("MYSQL_DATABASE").unwrap_or_else(|_| "is-by".to_string());
        
        let db_url = format!("mysql://{}:{}@{}:3306/{}", mysql_user, mysql_password, mysql_host, mysql_db);
        let pool = sqlx::mysql::MySqlPoolOptions::new().connect(&db_url).await.unwrap();

        let subs = sqlx::query_as::<_, PushSubscriptionRow>(
            "SELECT endpoint, p256dh, auth FROM push_subscriptions LIMIT 1"
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        if subs.is_empty() {
            println!("No subscriptions found in database!");
            return;
        }

        let sub = &subs[0];
        println!("Sending to endpoint: {}", sub.endpoint);

        let subscription_info = SubscriptionInfo::new(
            sub.endpoint.clone(),
            sub.p256dh.clone(),
            sub.auth.clone()
        );

        let mut sig_builder = VapidSignatureBuilder::from_base64(
            vapid_private.trim(),
            &subscription_info,
        ).unwrap();
        sig_builder.add_claim("sub", "mailto:admin@is-by.pro");
        let vapid_sig = sig_builder.build().unwrap();

        let payload_str = serde_json::json!({
            "title": "Test from CLI",
            "body": "This is a test web push notification.",
            "url": "/"
        }).to_string();

        let mut builder = WebPushMessageBuilder::new(&subscription_info);
        builder.set_payload(web_push::ContentEncoding::Aes128Gcm, payload_str.as_bytes());
        builder.set_vapid_signature(vapid_sig);
        builder.set_ttl(86400);

        let message = builder.build().unwrap();
        let client = IsahcWebPushClient::new().unwrap();

        match client.send(message).await {
            Ok(_) => println!("PUSH SENT SUCCESSFULLY!"),
            Err(e) => println!("PUSH FAILED: {:?}", e),
        }
    }
}

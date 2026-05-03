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
            &state.vapid_private_key,
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

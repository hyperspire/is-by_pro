use serde_json::{Value, json};
use crate::DOMAIN;

pub fn paypal_base_url() -> String {
  std::env::var("PAYPAL_BASE_URL").unwrap_or_else(|_| "https://api-m.sandbox.paypal.com".to_string())
}
pub async fn paypal_access_token() -> Result<String, String> {
  let client_id = std::env::var("PAYPAL_CLIENT_ID")
    .map_err(|_| "Missing PAYPAL_CLIENT_ID".to_string())?;
  let client_secret = std::env::var("PAYPAL_CLIENT_SECRET")
    .map_err(|_| "Missing PAYPAL_CLIENT_SECRET".to_string())?;

  let endpoint = format!("{}/v1/oauth2/token", paypal_base_url().trim_end_matches('/'));
  let response = reqwest::Client::new()
    .post(&endpoint)
    .basic_auth(&client_id, Some(client_secret))
    .form(&[("grant_type", "client_credentials")])
    .send()
    .await
    .map_err(|e| format!("PayPal token request failed: {}", e))?;

  if !response.status().is_success() {
    let body = response.text().await.unwrap_or_default();
    let client_id_preview = if client_id.len() >= 6 {
      format!("{}...", &client_id[..6])
    } else {
      "<short>".to_string()
    };
    return Err(format!(
      "PayPal token request rejected from {} (client_id={}, len={}): {}",
      endpoint,
      client_id_preview,
      client_id.len(),
      body
    ));
  }

  let json: Value = response
    .json()
    .await
    .map_err(|e| format!("PayPal token parse failed: {}", e))?;

  json
    .get("access_token")
    .and_then(Value::as_str)
    .map(|value| value.to_string())
    .ok_or_else(|| "PayPal token missing access_token".to_string())
}
pub async fn paypal_create_subscription(imageid: i64) -> Result<String, String> {
  let token = paypal_access_token().await?;
  let endpoint = format!("{}/v1/billing/subscriptions", paypal_base_url().trim_end_matches('/'));
  let plan_id = std::env::var("PAYPAL_PLAN_ID")
    .map_err(|_| "Missing PAYPAL_PLAN_ID (monthly billing plan id)".to_string())?;

  let body = json!({
    "plan_id": plan_id,
    "custom_id": imageid.to_string(),
    "application_context": {
      "return_url": format!("https://{}/v1/ads/paypal/return", DOMAIN),
      "cancel_url": format!("https://{}/v1/ads/paypal/cancel", DOMAIN)
    }
  });

  let response = reqwest::Client::new()
    .post(endpoint)
    .bearer_auth(token)
    .json(&body)
    .send()
    .await
    .map_err(|e| format!("PayPal create subscription failed: {}", e))?;

  if !response.status().is_success() {
    let body = response.text().await.unwrap_or_default();
    return Err(format!("PayPal create subscription rejected: {}", body));
  }

  let json: Value = response
    .json()
    .await
    .map_err(|e| format!("PayPal create subscription parse failed: {}", e))?;

  let approve_url = json
    .get("links")
    .and_then(Value::as_array)
    .and_then(|links| {
      links.iter().find_map(|item| {
        let is_approve = item.get("rel").and_then(Value::as_str) == Some("approve");
        if is_approve {
          item.get("href").and_then(Value::as_str)
        } else {
          None
        }
      })
    })
    .ok_or_else(|| "PayPal create subscription missing approval URL".to_string())?;

  Ok(approve_url.to_string())
}

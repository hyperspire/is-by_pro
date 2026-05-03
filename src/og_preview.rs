use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use regex::Regex;
use redis::AsyncCommands;
use crate::models::AppState;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct OgPreviewResponse {
    pub success: bool,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub url: String,
}

#[derive(serde::Deserialize)]
pub struct OgPreviewQuery {
    pub url: String,
}

#[get("/v1/og_preview")]
pub async fn get_og_preview(
    _req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<OgPreviewQuery>,
) -> impl Responder {
    let url = query.url.clone();
    
    let cache_key = format!("og:{}", url);
    let mut redis_conn = state.redis_pool.clone();
    
    if let Ok(cached) = redis_conn.get::<_, String>(&cache_key).await {
        if let Ok(parsed) = serde_json::from_str::<OgPreviewResponse>(&cached) {
            return HttpResponse::Ok().json(parsed);
        }
    }
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_default();
        
    let res = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => {
            return HttpResponse::Ok().json(OgPreviewResponse {
                success: false,
                title: None,
                description: None,
                image: None,
                url,
            });
        }
    };
    
    let html = res.text().await.unwrap_or_default();
    
    lazy_static::lazy_static! {
        static ref TITLE_RE: Regex = Regex::new(r#"(?i)<meta\s+(?:property|name)="og:title"\s+content="([^"]+)"#).unwrap();
        static ref DESC_RE: Regex = Regex::new(r#"(?i)<meta\s+(?:property|name)="og:description"\s+content="([^"]+)"#).unwrap();
        static ref IMG_RE: Regex = Regex::new(r#"(?i)<meta\s+(?:property|name)="og:image"\s+content="([^"]+)"#).unwrap();
        static ref HTML_TITLE_RE: Regex = Regex::new(r#"(?i)<title>([^<]+)</title>"#).unwrap();
    }
    
    let title = TITLE_RE.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
        .or_else(|| HTML_TITLE_RE.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()));
        
    let description = DESC_RE.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
    let image = IMG_RE.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
    
    let response = OgPreviewResponse {
        success: title.is_some() || description.is_some() || image.is_some(),
        title,
        description,
        image,
        url: url.clone(),
    };
    
    if let Ok(json_str) = serde_json::to_string(&response) {
        let _: Result<(), _> = redis_conn.set_ex(&cache_key, json_str, 7 * 24 * 3600).await;
    }
    
    HttpResponse::Ok().json(response)
}

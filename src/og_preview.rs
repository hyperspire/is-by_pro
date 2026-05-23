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
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(5))
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
    
    let mut title = None;
    let mut description = None;
    let mut image = None;

    lazy_static::lazy_static! {
        static ref META_RE: Regex = Regex::new(r#"(?i)<meta\s+([^>]+)>"#).unwrap();
        static ref PROP_RE: Regex = Regex::new(r#"(?i)(?:property|name)="([^"]+)""#).unwrap();
        static ref CONTENT_RE: Regex = Regex::new(r#"(?i)content="([^"]*)""#).unwrap();
        static ref HTML_TITLE_RE: Regex = Regex::new(r#"(?i)<title>\s*([^<]+)\s*</title>"#).unwrap();
    }

    for cap in META_RE.captures_iter(&html) {
        if let Some(attrs_match) = cap.get(1) {
            let attrs = attrs_match.as_str();
            if let Some(prop_cap) = PROP_RE.captures(attrs) {
                let prop = prop_cap.get(1).unwrap().as_str().to_lowercase();
                if prop == "og:title" || prop == "twitter:title" {
                    if title.is_none() {
                        if let Some(content_cap) = CONTENT_RE.captures(attrs) {
                            title = Some(content_cap.get(1).unwrap().as_str().to_string());
                        }
                    }
                } else if prop == "og:description" || prop == "twitter:description" {
                    if description.is_none() {
                        if let Some(content_cap) = CONTENT_RE.captures(attrs) {
                            description = Some(content_cap.get(1).unwrap().as_str().to_string());
                        }
                    }
                } else if prop == "og:image" || prop == "twitter:image" {
                    if image.is_none() {
                        if let Some(content_cap) = CONTENT_RE.captures(attrs) {
                            image = Some(content_cap.get(1).unwrap().as_str().to_string());
                        }
                    }
                }
            }
        }
    }

    if title.is_none() {
        title = HTML_TITLE_RE.captures(&html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
    }
    
    let response = OgPreviewResponse {
        success: title.is_some() || description.is_some() || image.is_some(),
        title,
        description,
        image,
        url: url.clone(),
    };
    
    if response.success {
        if let Ok(json_str) = serde_json::to_string(&response) {
            let _: Result<(), _> = redis_conn.set_ex(&cache_key, json_str, 7 * 24 * 3600).await;
        }
    }
    
    HttpResponse::Ok().json(response)
}

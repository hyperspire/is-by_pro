use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct GithubRepoQuery {
    pub owner: String,
    pub repo: String,
}

pub async fn get_github_repo_info(
    query: web::Query<GithubRepoQuery>,
    state: web::Data<crate::AppState>,
) -> impl Responder {
    let cache_key = format!("cache:github_repo:{}:{}", query.owner, query.repo);
    
    if let Some(cached_data) = crate::utils::get_cache(&state.redis_pool, &cache_key).await {
        return HttpResponse::Ok()
            .content_type("application/json")
            .body(cached_data);
    }

    let url = format!("https://api.github.com/repos/{}/{}", query.owner, query.repo);
    let client = reqwest::Client::new();
    
    match client
        .get(&url)
        .header("User-Agent", "is-by-pro")
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(json_val) = response.json::<serde_json::Value>().await {
                    let mut info = json!({});
                    info["name"] = json_val["name"].clone();
                    info["description"] = json_val["description"].clone();
                    info["stargazers_count"] = json_val["stargazers_count"].clone();
                    info["forks_count"] = json_val["forks_count"].clone();
                    info["language"] = json_val["language"].clone();
                    info["owner_avatar_url"] = json_val["owner"]["avatar_url"].clone();

                    let response_json = info.to_string();
                    crate::utils::set_cache(&state.redis_pool, &cache_key, &response_json, 3600).await;
                    
                    HttpResponse::Ok()
                        .content_type("application/json")
                        .body(response_json)
                } else {
                    HttpResponse::InternalServerError().body("Failed to parse GitHub response")
                }
            } else if response.status() == 404 {
                HttpResponse::NotFound().body("Repository not found")
            } else {
                HttpResponse::InternalServerError().body("GitHub API request failed")
            }
        }
        Err(_) => HttpResponse::InternalServerError().body("Failed to connect to GitHub"),
    }
}

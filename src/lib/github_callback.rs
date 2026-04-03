pub mod github_callback;

use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use crate::config::{GITHUB_CLIENT_ID, GITHUB_CLIENT_SECRET, GITHUB_REDIRECT_URI};
use actix_files::Files;

#[derive(Clone)]
struct AppState {
    github_client_id: String,
    github_client_secret: String,
    github_redirect_uri: String,
}

#[derive(Deserialize)]
struct GithubCallback {
    code: String,
    state: String,
}

#[derive(Deserialize)]
struct GithubTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct GithubUser {
    login: String,
    id: u64,
}


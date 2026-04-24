use actix_web::{cookie::Cookie, HttpRequest};
use crate::models::*;
use crate::{AD_ADMIN_UID, AD_ADMIN_USER};

pub fn get_session_uid(req: &HttpRequest) -> Option<i64> {
  req.cookie("ib_uid")
    .and_then(|cookie| cookie.value().parse::<i64>().ok())
}
pub fn is_expected_ad_admin_identity(ib_uid: i64, ib_user: &str) -> bool {
  ib_uid == AD_ADMIN_UID && ib_user == AD_ADMIN_USER
}
pub async fn is_ad_admin_session(req: &HttpRequest, state: &AppState) -> bool {
  let Some(session_uid) = get_session_uid(req) else {
    return false;
  };

  if session_uid != AD_ADMIN_UID {
    return false;
  }

  let session_username = match sqlx::query_as::<_, SessionUserRow>(
    "SELECT username FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1",
  )
  .bind(session_uid.to_string())
  .fetch_optional(&state.db_pool)
  .await
  {
    Ok(Some(row)) if !row.username.trim().is_empty() => row.username,
    _ => req
      .cookie("ib_user")
      .map(|cookie| cookie.value().to_string())
      .unwrap_or_default(),
  };

  session_username == AD_ADMIN_USER
}
pub async fn get_session_identity(req: &HttpRequest, state: &AppState) -> Option<(i64, String)> {
  let session_uid = get_session_uid(req)?;

  let username = match sqlx::query_as::<_, SessionUserRow>(
    "SELECT username FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1",
  )
  .bind(session_uid.to_string())
  .fetch_optional(&state.db_pool)
  .await
  {
    Ok(Some(row)) if !row.username.trim().is_empty() => row.username,
    _ => req
      .cookie("ib_user")
      .map(|cookie| cookie.value().to_string())
      .unwrap_or_default(),
  };

  if username.trim().is_empty() {
    None
  } else {
    Some((session_uid, username))
  }
}
pub fn remove_cookie(name: &str) -> Cookie<'static> {
  let mut cookie = Cookie::build(name.to_string(), String::new())
    .path("/")
    .http_only(true)
    .secure(true)
    .finish();
  cookie.make_removal();
  cookie
}

use crate::{models::*, db::*, utils::*, auth::*, render::*, crypto::*, paypal::*};
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder, Either, cookie::Cookie};
use serde_json::{Value, json};
use actix_multipart::Multipart;
use tera::Context;
use futures_util::StreamExt;
use uuid::Uuid;
use std::fs::{self};
use image::GenericImageView;
use rand::{distributions::Alphanumeric, Rng};
use crate::{DOMAIN, AD_ADMIN_UID, AD_ADMIN_USER};

pub async fn ensure_legacy_user_from_github(
  state: &AppState,
  github_id: u64,
  github_username: &str,
) -> Result<(), sqlx::Error> {
  // Keep the legacy user row in sync so profile redirects resolve by current GitHub login.
  sqlx::query(
    "INSERT INTO user (ib_uid, username, followers) VALUES (?, ?, '') ON DUPLICATE KEY UPDATE username = VALUES(username)",
  )
    .bind(github_id.to_string())
    .bind(github_username)
    .execute(&state.db_pool)
    .await?;



  Ok(())
}
#[post("/v1/post")]
pub async fn create_post(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Json<NewPostRequest>,
) -> impl Responder {
  const MAX_POST_LEN: usize = 1024;

  let Some((session_uid, session_username)) = get_session_identity(&req, &state).await else {
    return HttpResponse::Unauthorized().json(PostResponse {
      success: false,
      message: "Login required".to_string(),
      postid: None,
    });
  };

  if payload.post.trim().is_empty() {
    return HttpResponse::BadRequest().json(PostResponse {
      success: false,
      message: "Post cannot be empty".to_string(),
      postid: None,
    });
  }

  if payload.post.chars().count() > MAX_POST_LEN {
    return HttpResponse::BadRequest().json(PostResponse {
      success: false,
      message: format!("Post cannot exceed {} characters", MAX_POST_LEN),
      postid: None,
    });
  }

  let postid = Uuid::new_v4().to_string();

  // CREATE TABLE post (ib_uid varchar(64), postid varchar(64), parentid varchar(64), post varchar(1024), timestamp varchar(32));
  // ALTER TABLE post MODIFY COLUMN parentid VARCHAR(64) NOT NULL DEFAULT '';
  let result = sqlx::query(
    "INSERT INTO post (ib_uid, postid, post, `timestamp`) VALUES (?, ?, ?, NOW())",
  )
  .bind(session_uid)
  .bind(&postid)
  .bind(&payload.post)
  .execute(&state.db_pool)
  .await;

  match result {
    Ok(_) => {
      if let Err(err) = replace_post_tags(&state.db_pool, &postid, &payload.post).await {
        eprintln!("Post tag sync failed for {}: {}", postid, err);
      }

      let mentioned_users = extract_mentions(&payload.post);
      if !mentioned_users.is_empty() {
        for mentioned_user in mentioned_users {
          let target = match lookup_user_by_username(&state, &mentioned_user).await {
            Ok(Some(found)) => found,
            Ok(None) => continue,
            Err(err) => {
              eprintln!("Mention lookup failed for @{}: {}", mentioned_user, err);
              continue;
            }
          };

          let target_uid = target.0;
          if target_uid == session_uid {
            continue;
          }

          let dm_message = format!(
            "You were mentioned by @{} in a post:\n\n{}\n\n|||LINK|||https://{}/v1/showpost?ib_uid={}&ib_user={}&pid={}|||View Post|||",
            session_username,
            payload.post,
            DOMAIN,
            session_uid,
            url_encode_component(&session_username),
            postid
          );

          let stored_dm_message = match encode_dm_message_for_storage(&dm_message) {
            Ok(value) => value,
            Err(err) => {
              eprintln!(
                "Mention DM encryption failed from {} to {} for post {}: {}",
                session_uid,
                target_uid,
                postid,
                err
              );
              continue;
            }
          };

          if let Err(err) = sqlx::query(
            "INSERT INTO dm (sender_uid, recipient_uid, message) VALUES (?, ?, ?)",
          )
          .bind(session_uid)
          .bind(target_uid)
          .bind(stored_dm_message)
          .execute(&state.db_pool)
          .await
          {
            eprintln!(
              "Mention DM send failed from {} to {} for post {}: {}",
              session_uid,
              target_uid,
              postid,
              err
            );
          }
        }
      }

      HttpResponse::Ok()
        .insert_header(("ib_user", session_username.clone()))
        .json(PostResponse {
          success: true,
          message: "Post created".to_string(),
          postid: Some(postid),
        })
    }
    Err(err) => HttpResponse::InternalServerError().json(PostResponse {
      success: false,
      message: format!("Failed to create post: {}", err),
      postid: None,
    }),
  }
}
#[post("/v1/reply")]
pub async fn create_reply(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<ReplyRequest>,
) -> impl Responder {
  const MAX_POST_LEN: usize = 1024;

  let Some((session_uid, _)) = get_session_identity(&req, &state).await else {
    return HttpResponse::Unauthorized().body("Login required");
  };

  if payload.post.trim().is_empty() {
    return HttpResponse::BadRequest().body("Reply cannot be empty");
  }

  if payload.post.chars().count() > MAX_POST_LEN {
    return HttpResponse::BadRequest()
      .body(format!("Reply cannot exceed {} characters", MAX_POST_LEN));
  }

  let replyid = Uuid::new_v4().to_string();

  let result = sqlx::query(
    "INSERT INTO post (ib_uid, postid, parentid, post, `timestamp`) VALUES (?, ?, ?, ?, NOW())",
  )
  .bind(session_uid)
  .bind(&replyid)
  .bind(&payload.pid)
  .bind(&payload.post)
  .execute(&state.db_pool)
  .await;

  match result {
    Ok(_) => {
      if let Err(err) = replace_post_tags(&state.db_pool, &replyid, &payload.post).await {
        eprintln!("Reply tag sync failed for {}: {}", replyid, err);
      }

      HttpResponse::SeeOther()
        .insert_header((
          "Location",
          format!(
            "/v1/showpost?ib_uid={}&ib_user={}&pid={}",
            payload.ib_uid, payload.ib_user, payload.pid
          ),
        ))
        .finish()
    }
    Err(err) => HttpResponse::InternalServerError()
      .body(format!("Failed to create reply: {}", err)),
  }
}
#[post("/v1/showpost")]
pub async fn show_post(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<ShowPostRequest>,
) -> impl Responder {
  render_show_post_response(
    &req,
    &state,
    payload.ib_uid,
    &payload.ib_user,
    &payload.pid,
    get_session_uid(&req),
  )
  .await
}
#[get("/v1/showpost")]
pub async fn show_post_get(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<ShowPostRequest>,
) -> impl Responder {
  render_show_post_response(
    &req,
    &state,
    query.ib_uid,
    &query.ib_user,
    &query.pid,
    get_session_uid(&req),
  )
  .await
}
#[post("/v1/pinpost")]
pub async fn pin_post(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Json<PinPostRequest>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().json(PostResponse {
      success: false,
      message: "Unauthorized".to_string(),
      postid: None,
    }),
  };

  // Ensure the post being pinned belongs to the user
  if payload.ib_uid != session_uid {
    return HttpResponse::Forbidden().json(PostResponse {
      success: false,
      message: "You can only pin your own posts".to_string(),
      postid: None,
    });
  }

  // Get current pinned post
  let current_pinned: Option<String> = match sqlx::query_scalar::<_, Option<String>>(
    "SELECT pinned_postid FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1"
  )
  .bind(session_uid.to_string())
  .fetch_one(&state.db_pool)
  .await {
    Ok(val) => val,
    Err(err) => { eprintln!("pin_post fetch error: {:?}", err); None },
  };

  // Toggle logic: if already pinned, unpin. Otherwise pin.
  eprintln!("current_pinned: {:?}, payload.pid: {:?}", current_pinned, payload.pid);
  let pid_trimmed = payload.pid.trim();
  let current_pinned_trimmed = current_pinned.as_deref().map(|s| s.trim());
  let new_pinned = if current_pinned_trimmed == Some(pid_trimmed) {
    None
  } else {
    Some(pid_trimmed)
  };

  let update_result = sqlx::query(
    "UPDATE user SET pinned_postid = ? WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ?"
  )
  .bind(new_pinned)
  .bind(session_uid.to_string())
  .execute(&state.db_pool)
  .await;

  match update_result {
    Ok(_) => HttpResponse::Ok().json(PostResponse {
      success: true,
      message: if new_pinned.is_some() { "Post pinned".to_string() } else { "Post unpinned".to_string() },
      postid: Some(payload.pid.clone()),
    }),
    Err(e) => HttpResponse::InternalServerError().json(PostResponse {
      success: false,
      message: format!("Failed to update pinned post: {}", e),
      postid: None,
    }),
  }
}
#[get("/v1/embedpost")]
pub async fn embed_post_get(
  state: web::Data<AppState>,
  query: web::Query<ShowPostRequest>,
) -> impl Responder {
  render_embed_post_response(
    &state,
    query.ib_uid,
    &query.ib_user,
    &query.pid,
  )
  .await
}
#[post("/v1/follow")]
pub async fn follow_user(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<FollowRequest>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().body("Login required"),
  };

  let target_user = payload.target_user.trim();
  if target_user.is_empty() {
    return HttpResponse::BadRequest().body("Target user is required");
  }

  let target_row = match sqlx::query_as::<_, FollowLookupRow>(
    "SELECT username, COALESCE(followers, '') AS followers FROM user WHERE LOWER(username) = LOWER(?) LIMIT 1",
  )
  .bind(target_user)
  .fetch_optional(&state.db_pool)
  .await
  {
    Ok(Some(row)) => row,
    Ok(None) => return HttpResponse::NotFound().body("Profile not found"),
    Err(err) => {
      return HttpResponse::InternalServerError().body(format!("Follow lookup failed: {}", err));
    }
  };

  let follower_username = if let Some(cookie_user) = req.cookie("ib_user") {
    let value = cookie_user.value().trim();
    if !value.is_empty() {
      value.to_string()
    } else {
      String::new()
    }
  } else {
    String::new()
  };

  let follower_username = if !follower_username.is_empty() {
    follower_username
  } else {
    match sqlx::query_as::<_, SessionUserRow>(
      "SELECT username FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1",
    )
    .bind(session_uid.to_string())
    .fetch_optional(&state.db_pool)
    .await
    {
      Ok(Some(row)) if !row.username.trim().is_empty() => row.username,
      Ok(_) => return HttpResponse::Unauthorized().body("Could not resolve session username"),
      Err(err) => {
        return HttpResponse::InternalServerError().body(format!("Session user lookup failed: {}", err));
      }
    }
  };

  if follower_username.eq_ignore_ascii_case(&target_row.username) {
    return HttpResponse::SeeOther()
      .insert_header(("Location", format!("/v1/profile/{}", target_row.username)))
      .finish();
  }

  let mut followers: Vec<String> = target_row
    .followers
    .split(',')
    .map(|value| value.trim())
    .filter(|value| !value.is_empty())
    .map(|value| value.to_string())
    .collect();

  let already_following = followers
    .iter()
    .any(|value| value.eq_ignore_ascii_case(&follower_username));

  if !already_following {
    followers.push(follower_username);
  }

  let updated_followers = followers.join(", ");

  if let Err(err) = sqlx::query(
    "UPDATE user SET followers = ? WHERE LOWER(username) = LOWER(?) LIMIT 1",
  )
  .bind(updated_followers)
  .bind(&target_row.username)
  .execute(&state.db_pool)
  .await
  {
    return HttpResponse::InternalServerError().body(format!("Failed to update followers: {}", err));
  }

  HttpResponse::SeeOther()
    .insert_header(("Location", format!("/v1/profile/{}", target_row.username)))
    .finish()
}
#[get("/v1/user/hover/{ib_user}")]
pub async fn user_hover_card_data(
  req: HttpRequest,
  path: web::Path<String>,
  state: web::Data<AppState>,
) -> impl Responder {
  let ib_user = path.into_inner();
  let target_user = ib_user.trim();
  if target_user.is_empty() {
    return HttpResponse::BadRequest().json(json!({
      "success": false,
      "message": "Target user is required"
    }));
  }

  let target_row = match sqlx::query_as::<_, UserHoverLookupRow>(
    "SELECT CONVERT(ib_uid USING utf8mb4) AS ib_uid, username, COALESCE(followers, '') AS followers, COALESCE(total_acknowledgments, 0) AS total_acknowledgments FROM user WHERE LOWER(username) = LOWER(?) LIMIT 1",
  )
  .bind(target_user)
  .fetch_optional(&state.db_pool)
  .await
  {
    Ok(Some(row)) => row,
    Ok(None) => {
      return HttpResponse::NotFound().json(json!({
        "success": false,
        "message": "User not found"
      }));
    }
    Err(err) => {
      return HttpResponse::InternalServerError().json(json!({
        "success": false,
        "message": format!("Hover lookup failed: {}", err)
      }));
    }
  };

  let session_username = get_session_identity(&req, &state)
    .await
    .map(|(_, username)| username);

  let follower_list: Vec<String> = target_row
    .followers
    .split(',')
    .map(|value| value.trim())
    .filter(|value| !value.is_empty())
    .map(|value| value.to_string())
    .collect();

  let is_self = session_username
    .as_ref()
    .map(|value| value.eq_ignore_ascii_case(&target_row.username))
    .unwrap_or(false);

  let show_unfollow = session_username
    .as_ref()
    .map(|value| {
      !is_self
        && follower_list
          .iter()
          .any(|follower| follower.eq_ignore_ascii_case(value))
    })
    .unwrap_or(false);

  let show_follow = session_username
    .as_ref()
    .map(|value| {
      !is_self
        && !follower_list
          .iter()
          .any(|follower| follower.eq_ignore_ascii_case(value))
    })
    .unwrap_or(false);

  let (rank_level, rank_name) = rank_from_unique_acknowledgments(target_row.total_acknowledgments);
  let rank_icon = get_rank_asset(target_row.total_acknowledgments);

  HttpResponse::Ok().json(json!({
    "success": true,
    "ib_uid": target_row.ib_uid,
    "username": target_row.username,
    "total_acknowledgments": target_row.total_acknowledgments,
    "unique_acknowledgments": target_row.total_acknowledgments,
    "rank_level": rank_level,
    "rank_name": rank_name,
    "rank_icon": format!("/images/ranks/{}", rank_icon),
    "show_follow": show_follow,
    "show_unfollow": show_unfollow,
    "logged_in": session_username.is_some()
  }))
}
#[get("/api/badge/{username}.png")]
pub async fn get_commander_badge(
  path: web::Path<String>,
  state: web::Data<AppState>,
) -> impl Responder {
  let username = path.into_inner();
  let target_user = username.trim();
  if target_user.is_empty() {
    return Either::Left(HttpResponse::BadRequest().body("Username is required"));
  }

  // Query user to check rank
  let user_row = match sqlx::query_as::<_, UserHoverLookupRow>(
    "SELECT CONVERT(ib_uid USING utf8mb4) AS ib_uid, username, COALESCE(followers, '') AS followers, COALESCE(total_acknowledgments, 0) AS total_acknowledgments FROM user WHERE LOWER(username) = LOWER(?) LIMIT 1",
  )
  .bind(target_user)
  .fetch_optional(&state.db_pool)
  .await
  {
    Ok(Some(row)) => row,
    Ok(None) => return Either::Left(HttpResponse::NotFound().body("User not found")),
    Err(_) => return Either::Left(HttpResponse::InternalServerError().body("Database error")),
  };

  // Check if user is Rank 10+ (threshold >= 9001)
  let (rank_level, _) = rank_from_unique_acknowledgments(user_row.total_acknowledgments);
  if rank_level < 10 {
    // Return a transparent 1x1 pixel instead of an error to prevent broken image icons on GitHub.
    let transparent_pixel: &[u8] = &[
      0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
      0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
      0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0x60, 0x60, 0x60, 0x00,
      0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
      0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    return Either::Right(
      HttpResponse::Ok()
        .content_type("image/png")
        .insert_header(("Cache-Control", "no-cache, no-store, must-revalidate"))
        .body(transparent_pixel)
    );
  }

  // Return PNG badge image
  match fs::read("./webroot/images/hall_of_heroes.png") {
    Ok(image_data) => {
      Either::Right(
        HttpResponse::Ok()
          .content_type("image/png")
          .insert_header(("Access-Control-Allow-Origin", "*"))
          .insert_header(("Cache-Control", "public, max-age=3600"))
          .insert_header(("Content-Disposition", "inline"))
          .body(image_data)
      )
    }
    Err(_) => {
      Either::Left(HttpResponse::InternalServerError().body("Badge image not found"))
    }
  }
}
#[post("/v1/unfollow")]
pub async fn unfollow_user(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<FollowRequest>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().body("Login required"),
  };

  let target_user = payload.target_user.trim();
  if target_user.is_empty() {
    return HttpResponse::BadRequest().body("Target user is required");
  }

  let target_row = match sqlx::query_as::<_, FollowLookupRow>(
    "SELECT username, COALESCE(followers, '') AS followers FROM user WHERE LOWER(username) = LOWER(?) LIMIT 1",
  )
  .bind(target_user)
  .fetch_optional(&state.db_pool)
  .await
  {
    Ok(Some(row)) => row,
    Ok(None) => return HttpResponse::NotFound().body("Profile not found"),
    Err(err) => {
      return HttpResponse::InternalServerError().body(format!("Unfollow lookup failed: {}", err));
    }
  };

  let follower_username = if let Some(cookie_user) = req.cookie("ib_user") {
    let value = cookie_user.value().trim();
    if !value.is_empty() {
      value.to_string()
    } else {
      String::new()
    }
  } else {
    String::new()
  };

  let follower_username = if !follower_username.is_empty() {
    follower_username
  } else {
    match sqlx::query_as::<_, SessionUserRow>(
      "SELECT username FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1",
    )
    .bind(session_uid.to_string())
    .fetch_optional(&state.db_pool)
    .await
    {
      Ok(Some(row)) if !row.username.trim().is_empty() => row.username,
      Ok(_) => return HttpResponse::Unauthorized().body("Could not resolve session username"),
      Err(err) => {
        return HttpResponse::InternalServerError().body(format!("Session user lookup failed: {}", err));
      }
    }
  };

  let followers: Vec<String> = target_row
    .followers
    .split(',')
    .map(|value| value.trim())
    .filter(|value| !value.is_empty())
    .filter(|value| !value.eq_ignore_ascii_case(&follower_username))
    .map(|value| value.to_string())
    .collect();

  let updated_followers = followers.join(", ");

  if let Err(err) = sqlx::query(
    "UPDATE user SET followers = ? WHERE LOWER(username) = LOWER(?) LIMIT 1",
  )
  .bind(updated_followers)
  .bind(&target_row.username)
  .execute(&state.db_pool)
  .await
  {
    return HttpResponse::InternalServerError().body(format!("Failed to update followers: {}", err));
  }

  HttpResponse::SeeOther()
    .insert_header(("Location", format!("/v1/profile/{}", target_row.username)))
    .finish()
}

#[post("/v1/block")]
pub async fn block_user(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<FollowRequest>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().body("Login required"),
  };

  let target_user = payload.target_user.trim();
  if target_user.is_empty() {
    return HttpResponse::BadRequest().body("Target user is required");
  }

  let target_uid = match lookup_user_by_username(&state, target_user).await {
    Ok(Some((uid, _))) => uid,
    Ok(None) => return HttpResponse::NotFound().body("Profile not found"),
    Err(err) => return HttpResponse::InternalServerError().body(format!("Block lookup failed: {}", err)),
  };

  if session_uid == target_uid {
    return HttpResponse::BadRequest().body("Cannot block yourself");
  }

  if let Err(err) = sqlx::query(
    "INSERT IGNORE INTO user_blocks (blocker_uid, blocked_uid, created_at) VALUES (?, ?, NOW())",
  )
  .bind(session_uid)
  .bind(target_uid)
  .execute(&state.db_pool)
  .await
  {
    return HttpResponse::InternalServerError().body(format!("Failed to block user: {}", err));
  }

  HttpResponse::SeeOther()
    .insert_header(("Location", format!("/v1/profile/{}", url_encode_component(target_user))))
    .finish()
}

#[post("/v1/unblock")]
pub async fn unblock_user(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<FollowRequest>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().body("Login required"),
  };

  let target_user = payload.target_user.trim();
  if target_user.is_empty() {
    return HttpResponse::BadRequest().body("Target user is required");
  }

  let target_uid = match lookup_user_by_username(&state, target_user).await {
    Ok(Some((uid, _))) => uid,
    Ok(None) => return HttpResponse::NotFound().body("Profile not found"),
    Err(err) => return HttpResponse::InternalServerError().body(format!("Unblock lookup failed: {}", err)),
  };

  if let Err(err) = sqlx::query(
    "DELETE FROM user_blocks WHERE blocker_uid = ? AND blocked_uid = ?",
  )
  .bind(session_uid)
  .bind(target_uid)
  .execute(&state.db_pool)
  .await
  {
    return HttpResponse::InternalServerError().body(format!("Failed to unblock user: {}", err));
  }

  HttpResponse::SeeOther()
    .insert_header(("Location", format!("/v1/profile/{}", url_encode_component(target_user))))
    .finish()
}

#[get("/v1/editprofile")]
pub async fn edit_profile(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<EditProfileRequest>,
) -> impl Responder {
  let mut context = Context::new();
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().body("Login required"),
  };

  if session_uid != query.ib_uid {
    return HttpResponse::Forbidden().body("You can only edit your own profile");
  }

  let ib_uid = query.ib_uid.clone();
  let ib_user = query.ib_user.clone();

  let row = match sqlx::query_as::<_, EditProfileRow>(
    "SELECT github AS ib_github, ibp AS ib_ibp, pro AS ib_pro, services AS ib_services, location AS ib_location, website AS ib_website FROM pro WHERE ib_uid = ? LIMIT 1",
  )
  .bind(ib_uid)
  .fetch_optional(&state.db_pool)
  .await
  {
    Ok(Some(row)) => row,
    Ok(None) => EditProfileRow {
      ib_ibp: String::new(),
      ib_pro: String::new(),
      ib_services: String::new(),
      ib_location: String::new(),
      ib_website: String::new(),
    },
    Err(err) => {
      return HttpResponse::InternalServerError().body(format!("Edit profile lookup failed: {}", err));
    }
  };

  let advert_html = render_advert_html(&state).await;

  let edit_profile_html = format!(
    r#"<div id="post-form-section" style="display:block;">
        <form id="edit-profile-form" action="https://{DOMAIN}/v1/editprofile" method="POST">
          <input type="hidden" name="ib_uid" value="{ib_uid}">
          <input type="hidden" name="ib_user" value="{ib_user}">
          GitHub: https://github.com/<input class="post" type="text" name="ib_github" value="{ib_user}" autocomplete="off"><br>
          About Me: <input class="post" type="text" name="ib_ibp" value="{ib_ibp}" placeholder="{ib_ibp}" autocomplete="off"><br>
          Interests: <input class="post" type="text" name="ib_pro" value="{ib_pro}" placeholder="{ib_pro}" autocomplete="off"><br>
          Services: <input class="post" type="text" name="ib_services" value="{ib_services}" placeholder="{ib_services}" autocomplete="off"><br>
          Location: <input class="post" type="text" name="ib_location" value="{ib_location}" placeholder="{ib_location}" autocomplete="off"><br>
          Website: <input class="post" type="text" name="ib_website" value="{ib_website}" placeholder="{ib_website}" autocomplete="off"><br>
          <input class="post-submit" type="submit" value="Save">
        </form>
      </div>"#,
    ib_uid = query.ib_uid,
    ib_user = escape_html(&query.ib_user),
    ib_ibp = escape_html(&row.ib_ibp),
    ib_pro = escape_html(&row.ib_pro),
    ib_services = escape_html(&row.ib_services),
    ib_location = escape_html(&row.ib_location),
    ib_website = escape_html(&row.ib_website)
  );

  context.insert("advert_html", &advert_html);
  context.insert("edit_profile_html", &edit_profile_html);
  context.insert("domain", &DOMAIN);
  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &ib_user);

  let html = match TEMPLATES.render("edit_profile.html", &context) {
        Ok(rendered) => rendered,
        Err(e) => {
            use std::error::Error;
            let mut err_msg = format!("Template error: {}", e);
            let mut cause = e.source();
            while let Some(err) = cause {
                err_msg.push_str(&format!("\nCaused by: {}", err));
                cause = err.source();
            }
            return HttpResponse::InternalServerError().body(err_msg);
        }
  };

  HttpResponse::Ok()
    .content_type("text/html; charset=utf-8")
    .body(html)
}

#[post("/v1/editprofile")]
pub async fn update_profile(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<EditProfileUpdateRequest>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().body("Login required"),
  };

  if session_uid != payload.ib_uid {
    return HttpResponse::Forbidden().body("You can only edit your own profile");
  }

  let update_result = sqlx::query(
    "UPDATE pro SET github = ?, ibp = ?, pro = ?, services = ?, location = ?, website = ? WHERE ib_uid = ?",
  )
  .bind(&payload.ib_github)
  .bind(&payload.ib_ibp)
  .bind(&payload.ib_pro)
  .bind(&payload.ib_services)
  .bind(&payload.ib_location)
  .bind(&payload.ib_website)
  .bind(payload.ib_uid)
  .execute(&state.db_pool)
  .await;

  match update_result {
    Ok(update_done) if update_done.rows_affected() > 0 => HttpResponse::SeeOther()
      .insert_header(("Location", format!("/v1/profile/{}", payload.ib_user)))
      .finish(),
    Ok(_) => {
      let insert_result = sqlx::query(
        "INSERT INTO pro (ib_uid, github, ibp, pro, services, location, website) VALUES (?, ?, ?, ?, ?, ?, ?)",
      )
      .bind(payload.ib_uid)
      .bind(&payload.ib_github)
      .bind(&payload.ib_ibp)
      .bind(&payload.ib_pro)
      .bind(&payload.ib_services)
      .bind(&payload.ib_location)
      .bind(&payload.ib_website)
      .execute(&state.db_pool)
      .await;

      match insert_result {
        Ok(_) => HttpResponse::SeeOther()
          .insert_header(("Location", format!("/v1/profile/{}", payload.ib_user)))
          .finish(),
        Err(err) => HttpResponse::InternalServerError()
          .body(format!("Failed to create profile: {}", err)),
      }
    }
    Err(err) => HttpResponse::InternalServerError()
      .body(format!("Failed to update profile: {}", err)),
  }
}
#[post("/v1/deletepost")]
pub async fn delete_post(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<DeletePostRequest>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().body("Login required"),
  };

  let owner_uid = payload.post_owner_uid.unwrap_or(payload.ib_uid);
  if session_uid != owner_uid {
    return HttpResponse::Forbidden().body("You can only delete your own posts");
  }

  let result = sqlx::query(
    "DELETE FROM post WHERE ib_uid = ? AND postid = ?",
  )
  .bind(owner_uid)
  .bind(&payload.pid)
  .execute(&state.db_pool)
  .await;

  match result {
    Ok(_) => {
      if let Err(err) = remove_post_tags(&state.db_pool, &payload.pid).await {
        eprintln!("Post tag delete failed for {}: {}", payload.pid, err);
      }

      if let Err(err) = remove_post_acks(&state.db_pool, &payload.pid).await {
        eprintln!("Post ack delete failed for {}: {}", payload.pid, err);
      }

      let location = if let Some(root_pid) = payload.root_pid.as_deref() {
        format!(
          "/v1/showpost?ib_uid={}&ib_user={}&pid={}",
          payload.ib_uid, payload.ib_user, root_pid
        )
      } else {
        format!("/v1/profile/{}", payload.ib_user)
      };

      HttpResponse::SeeOther()
        .insert_header(("Location", location))
        .finish()
    }
    Err(err) => HttpResponse::InternalServerError()
      .body(format!("Failed to delete post: {}", err)),
  }
}

#[get("/v1/editpost")]
pub async fn edit_post(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<EditPostRequest>,
) -> impl Responder {
  let mut context = Context::new();
  let advert_html = render_advert_html(&state).await;
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().body("Login required"),
  };

  let ib_uid = query.ib_uid.clone();
  let ib_user = query.ib_user.clone();

  let owner_uid = query.post_owner_uid.unwrap_or(ib_uid);
  if session_uid != owner_uid {
    return HttpResponse::Forbidden().body("You can only edit your own posts");
  }

  let selected = sqlx::query_as::<_, EditPostRow>(
    "SELECT post FROM post WHERE ib_uid = ? AND postid = ? LIMIT 1",
  )
  .bind(owner_uid)
  .bind(&query.pid)
  .fetch_optional(&state.db_pool)
  .await;

  let row = match selected {
    Ok(Some(row)) => row,
    Ok(None) => {
      return HttpResponse::NotFound()
        .content_type("text/html; charset=utf-8")
        .body(format!(
          r#"<!DOCTYPE html><html lang="en-US"><head><meta charset="UTF-8"><title>Post Not Found</title></head><body><p>Post not found: {}</p></body></html>"#,
          escape_html(&query.pid)
        ));
    }
    Err(err) => {
      return HttpResponse::InternalServerError().body(format!("Edit post lookup failed: {}", err));
    }
  };

  let root_pid_field = query.root_pid.as_ref().map(|root_pid| {
    format!(
      r#"<input type="hidden" name="root_pid" value="{}">"#,
      escape_html(root_pid)
    )
  }).unwrap_or_default();

  let post_owner_uid_field = query.post_owner_uid.map(|post_owner_uid| {
    format!(
      r#"<input type="hidden" name="post_owner_uid" value="{}">"#,
      post_owner_uid
    )
  }).unwrap_or_default();

  let post_edit_html = format!(
    r#"<div id="post-form-section" style="display:block;">
        <form id="editpost" action="https://{DOMAIN}/v1/editpost" method="POST">
          <div id="edit-post-message"></div>
          <input type="hidden" name="ib_uid" value="{ib_uid}">
          <input type="hidden" name="ib_user" value="{ib_user}">
          <input type="hidden" name="pid" value="{pid}">
          {root_pid_field}
          {post_owner_uid_field}
          <input type="text" class="post" name="post" maxlength="1024" value="{post}" required>
          <br>
          <input class="post-submit" type="submit" value="Save">
        </form>
      </div>"#,
    ib_uid = query.ib_uid,
    ib_user = escape_html(&query.ib_user),
    pid = escape_html(&query.pid),
    root_pid_field = root_pid_field,
    post_owner_uid_field = post_owner_uid_field,
    post = escape_html(&row.post)
  );

  context.insert("advert_html", &advert_html);
  context.insert("post_edit_html", &post_edit_html);
  context.insert("domain", &DOMAIN);
  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &ib_user);

  let html = match TEMPLATES.render("edit_post.html", &context) {
        Ok(rendered) => rendered,
        Err(e) => {
            use std::error::Error;
            let mut err_msg = format!("Template error: {}", e);
            let mut cause = e.source();
            while let Some(err) = cause {
                err_msg.push_str(&format!("\nCaused by: {}", err));
                cause = err.source();
            }
            return HttpResponse::InternalServerError().body(err_msg);
        }
  };

  HttpResponse::Ok()
    .content_type("text/html; charset=utf-8")
    .body(html)
}

#[post("/v1/editpost")]
pub async fn update_post(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<EditPostUpdateRequest>,
) -> impl Responder {
  const MAX_POST_LEN: usize = 1024;

  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().body("Login required"),
  };

  let owner_uid = payload.post_owner_uid.unwrap_or(payload.ib_uid);
  if session_uid != owner_uid {
    return HttpResponse::Forbidden().body("You can only edit your own posts");
  }

  if payload.post.trim().is_empty() {
    return HttpResponse::BadRequest().body("Post cannot be empty");
  }

  if payload.post.chars().count() > MAX_POST_LEN {
    return HttpResponse::BadRequest()
      .body(format!("Post cannot exceed {} characters", MAX_POST_LEN));
  }

  let result = sqlx::query(
    "UPDATE post SET post = ? WHERE ib_uid = ? AND postid = ?",
  )
  .bind(&payload.post)
  .bind(owner_uid)
  .bind(&payload.pid)
  .execute(&state.db_pool)
  .await;

  match result {
    Ok(_) => {
      if let Err(err) = replace_post_tags(&state.db_pool, &payload.pid, &payload.post).await {
        eprintln!("Post tag update failed for {}: {}", payload.pid, err);
      }

      let location = if let Some(root_pid) = payload.root_pid.as_deref() {
        format!(
          "/v1/showpost?ib_uid={}&ib_user={}&pid={}",
          payload.ib_uid, payload.ib_user, root_pid
        )
      } else {
        format!("/v1/profile/{}", payload.ib_user)
      };

      HttpResponse::SeeOther()
        .insert_header(("Location", location))
        .finish()
    }
    Err(err) => HttpResponse::InternalServerError()
      .body(format!("Failed to update post: {}", err)),
  }
}

#[post("/v1/ackpost")]
pub async fn acknowledge_post(
  state: web::Data<AppState>,
  payload: web::Form<AckPostRequest>,
  req: HttpRequest,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().body("Login required"),
  };

  if session_uid != payload.ib_uid {
    return HttpResponse::Forbidden().body("Session mismatch");
  }

  let mut tx = match state.db_pool.begin().await {
    Ok(t) => t,
    Err(err) => return HttpResponse::InternalServerError().body(format!("Failed to start transaction: {}", err)),
  };

  let insert_result = sqlx::query("INSERT IGNORE INTO post_ack (postid, ib_uid) VALUES (?, ?)")
    .bind(&payload.pid)
    .bind(session_uid)
    .execute(&mut *tx)
    .await;

  match insert_result {
    Ok(res) if res.rows_affected() > 0 => {
      if let Err(err) = sqlx::query("UPDATE post SET acknowledged_count = COALESCE(acknowledged_count, 0) + 1 WHERE postid = ? LIMIT 1")
        .bind(&payload.pid)
        .execute(&mut *tx)
        .await {
          return HttpResponse::InternalServerError().body(format!("Failed to update post count: {}", err));
        }

      if let Err(err) = sqlx::query("UPDATE user SET total_acknowledgments = COALESCE(total_acknowledgments, 0) + 1 WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = (SELECT CAST(ib_uid AS CHAR CHARACTER SET utf8mb4) FROM post WHERE postid = ? LIMIT 1) COLLATE utf8mb4_unicode_ci LIMIT 1")
        .bind(&payload.pid)
        .execute(&mut *tx)
        .await {
          return HttpResponse::InternalServerError().body(format!("Failed to update user total: {}", err));
        }
    }
    Ok(_) => (), // Already acknowledged
    Err(err) => return HttpResponse::InternalServerError().body(format!("Failed to record acknowledgment: {}", err)),
  };

  if let Err(err) = tx.commit().await {
    return HttpResponse::InternalServerError().body(format!("Failed to commit acknowledgment: {}", err));
  }

  let fallback_location = format!(
    "/v1/showpost?ib_uid={}&ib_user={}&pid={}",
    payload.ib_uid,
    payload.ib_user,
    payload.pid
  );

  let location = req
    .headers()
    .get("referer")
    .and_then(|value| value.to_str().ok())
    .filter(|value| !value.trim().is_empty())
    .unwrap_or(&fallback_location)
    .to_string();

  HttpResponse::SeeOther()
    .insert_header(("Location", location))
    .finish()
}

#[get("/api/v1/posts")]
pub async fn get_posts_page(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<PostsPageQuery>,
) -> impl Responder {
  let session_uid = get_session_uid(&req);
  let ib_uid = query.ib_uid;
  let ib_user = &query.ib_user.clone();

  let is_blocked = crate::db::is_blocked(&state, session_uid, Some(ib_uid)).await;
  if is_blocked {
    return HttpResponse::Ok().json(PostsPageResponse {
      posts_html: String::new(),
      has_more: false,
    });
  }

  let ib_post_results = if let Some(before_ts) = &query.before_timestamp {
    sqlx::query_as::<_, PostRow>(
        "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE post.ib_uid = ? AND (post.parentid = '' OR post.parentid IS NULL) AND post.timestamp < ? ORDER BY post.timestamp DESC LIMIT 21"
      )
      .bind(ib_uid)
      .bind(before_ts)
      .fetch_all(&state.db_pool)
      .await
  } else {
    sqlx::query_as::<_, PostRow>(
        "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE post.ib_uid = ? AND (post.parentid = '' OR post.parentid IS NULL) ORDER BY post.timestamp DESC LIMIT 21"
      )
      .bind(ib_uid)
      .fetch_all(&state.db_pool)
      .await
  };

  let ib_post_results = match ib_post_results {
    Ok(rows) => rows,
    Err(e) => {
      return HttpResponse::InternalServerError().json(json!({
        "error": format!("Post query failed: {}", e)
      }));
    }
  };

  let has_more = ib_post_results.len() > 20;
  let display_rows = &ib_post_results[..ib_post_results.len().min(20)];

  let displayed_post_ids: Vec<String> = display_rows.iter().map(|row| row.postid.clone()).collect();
  let acknowledged_post_ids = acknowledged_post_ids_for_user(&state.db_pool, session_uid, &displayed_post_ids).await;

  let mut posts_html = String::new();
  for row in display_rows.iter() {
    let row_owner_uid = row.ib_uid.parse::<i64>().ok();
    let can_manage_post = session_uid.is_some() && session_uid == row_owner_uid;
    let manage_actions = if can_manage_post {
      format!(
        r#"<form class="delete-post-form" action="https://{DOMAIN}/v1/deletepost" method="POST">
            <input type="hidden" name="ib_uid" value="{ib_uid}">
            <input type="hidden" name="ib_user" value="{ib_user}">
            <input type="hidden" name="pid" value="{ib_post_id}">
          </form>
          <form class="edit-post-form" action="https://{DOMAIN}/v1/editpost" method="GET">
            <input type="hidden" name="ib_uid" value="{ib_uid}">
            <input type="hidden" name="ib_user" value="{ib_user}">
            <input type="hidden" name="pid" value="{ib_post_id}">
          </form>
          <a href="javascript:void(0);" class="edit-post">:[[ :edit: ]]:</a><a href="javascript:void(0);" class="delete-post">:[[ :delete: ]]:</a>"#,
        ib_uid = ib_uid,
        ib_user = escape_html(ib_user),
        ib_post_id = escape_html(&row.postid),
      )
    } else {
      String::new()
    };

    posts_html += &format!(
      r#"
      <div class="post" data-postid="{ib_post_id}" data-timestamp="{ib_post_timestamp}">
        {post_meta}
        <p>{post_body}</p>
        <div class="post-actions">
          {ack_controls}
          {manage_actions}
          <form class="show-post-form" action="https://{DOMAIN}/v1/showpost" method="GET">
            <input type="hidden" name="ib_uid" value="{ib_uid}">
            <input type="hidden" name="ib_user" value="{ib_user}">
            <input type="hidden" name="pid" value="{ib_post_id}">
          </form>
          <a href="javascript:void(0);" class="show-post">:[[ :show-post: ]]:</a>
        </div>
        <p class="acknowledged-count">Acknowleged {acknowledged_count} times.</p>
      </div>"#,
      ib_post_id = escape_html(&row.postid),
      ib_post_timestamp = escape_html(&row.timestamp),
      post_meta = render_post_meta(&row.ib_uid, &row.username, &row.timestamp, row.user_total_acks),
      manage_actions = manage_actions,
      ack_controls = if session_uid.is_none() || acknowledged_post_ids.contains(&row.postid) {
        render_ack_disabled()
      } else {
        render_ack_controls(ib_uid, ib_user, &row.postid)
      },
      acknowledged_count = row.acknowledged_count,
      post_body = render_post_with_hashtags(&row.post, ib_uid, ib_user),
      ib_uid = ib_uid,
      ib_user = escape_html(ib_user),
    );
  }

  HttpResponse::Ok().json(PostsPageResponse { posts_html, has_more })
}

#[get("/api/v1/warroom/posts")]
pub async fn get_war_room_posts_page(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<WarRoomPostsPageQuery>,
) -> impl Responder {
  let session_uid = get_session_uid(&req);
  let offset = query.offset.unwrap_or(0).max(0) as usize;
  let limit = query.limit.unwrap_or(20).clamp(1, 50) as usize;

  match render_war_room_posts_chunk(
    &state,
    query.ib_uid,
    &query.ib_user,
    session_uid,
    offset,
    limit,
  )
  .await
  {
    Ok(chunk) => HttpResponse::Ok().json(WarRoomPostsPageResponse {
      posts_html: chunk.posts_html,
      has_more: chunk.has_more,
      next_offset: chunk.next_offset,
    }),
    Err(e) => HttpResponse::InternalServerError().json(json!({
      "error": e
    })),
  }
}

#[get("/api/v1/followers")]
pub async fn get_followers_page(
  state: web::Data<AppState>,
  query: web::Query<FollowersPageQuery>,
) -> impl Responder {
  let offset = query.offset.unwrap_or(0).max(0) as usize;
  let limit = query.limit.unwrap_or(20).clamp(1, 50) as usize;

  match render_profile_followers_chunk(&state, query.ib_uid, offset, limit).await {
    Ok(chunk) => {
      if chunk.total_followers == 0 {
        return HttpResponse::Ok().json(FollowersPageResponse {
          followers_html: String::new(),
          has_more: false,
          next_offset: 0,
        });
      }

      HttpResponse::Ok().json(FollowersPageResponse {
        followers_html: chunk.followers_html,
        has_more: chunk.has_more,
        next_offset: chunk.next_offset,
      })
    }
    Err(e) => HttpResponse::InternalServerError().json(json!({
      "error": e
    })),
  }
}

#[post("/v1/projects")]
pub async fn create_project_profile(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<CreateProjectRequest>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().body("Login required"),
  };

  if session_uid != payload.ib_uid {
    return HttpResponse::Forbidden().body("Session mismatch");
  }

  if payload.project.trim().is_empty() || payload.description.trim().is_empty() || payload.languages.trim().is_empty() {
    return HttpResponse::BadRequest().body("Project, Description, and Languages are required");
  }

  let insert_result = sqlx::query(
    "INSERT INTO project_profile (ib_uid, project, description, languages, reinforcements, reinforcements_request) VALUES (?, ?, ?, ?, ?, ?)",
  )
  .bind(payload.ib_uid)
  .bind(payload.project.trim())
  .bind(payload.description.trim())
  .bind(payload.languages.trim())
  .bind(payload.reinforcements.as_ref().and_then(|s| if s.trim().is_empty() { None } else { Some(s.trim()) }))
  .bind(payload.reinforcements_request.as_ref().map(|s| !s.is_empty()).unwrap_or(false))
  .execute(&state.db_pool)
  .await;

  match insert_result {
    Ok(_) => HttpResponse::SeeOther()
      .insert_header(("Location", format!("/v1/projects?ib_uid={}&ib_user={}", payload.ib_uid, payload.ib_user)))
      .finish(),
    Err(err) => HttpResponse::InternalServerError().body(format!("Failed to create project: {}", err)),
  }
}

#[post("/v1/projects/edit")]
pub async fn update_project_profile(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<EditProjectRequest>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().body("Login required"),
  };

  if session_uid != payload.ib_uid {
    return HttpResponse::Forbidden().body("Session mismatch");
  }

  if payload.project.trim().is_empty() || payload.description.trim().is_empty() || payload.languages.trim().is_empty() {
    return HttpResponse::BadRequest().body("Project, Description, and Languages are required");
  }

  let update_result = sqlx::query(
    "UPDATE project_profile SET project = ?, description = ?, languages = ?, reinforcements = ?, reinforcements_request = ? WHERE id = ? AND ib_uid = ?",
  )
  .bind(payload.project.trim())
  .bind(payload.description.trim())
  .bind(payload.languages.trim())
  .bind(payload.reinforcements.as_ref().and_then(|s| if s.trim().is_empty() { None } else { Some(s.trim()) }))
  .bind(payload.reinforcements_request.as_ref().map(|s| !s.is_empty()).unwrap_or(false))
  .bind(payload.project_id)
  .bind(session_uid)
  .execute(&state.db_pool)
  .await;

  match update_result {
    Ok(_) => HttpResponse::SeeOther()
      .insert_header(("Location", format!("/v1/projects?ib_uid={}&ib_user={}", payload.ib_uid, payload.ib_user)))
      .finish(),
    Err(err) => HttpResponse::InternalServerError().body(format!("Failed to update project: {}", err)),
  }
}

#[post("/v1/projects/reinforce")]
pub async fn quick_response_force_project(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<QuickResponseForceRequest>,
) -> impl Responder {
  let Some((session_uid, session_username)) = get_session_identity(&req, &state).await else {
    return HttpResponse::Unauthorized().body("Login required");
  };

  if payload.quick_response_force.as_deref().unwrap_or_default().trim().is_empty() {
    return HttpResponse::BadRequest().body("Missing quick response action");
  }

  let quick_action = payload
    .quick_response_force
    .as_deref()
    .unwrap_or_default()
    .trim()
    .to_ascii_lowercase();

  let project_row = match sqlx::query_as::<_, ProjectReinforcementsRow>(
    "SELECT ib_uid, COALESCE(reinforcements, '') AS reinforcements, COALESCE(reinforcements_request, FALSE) AS reinforcements_request FROM project_profile WHERE id = ? LIMIT 1",
  )
  .bind(payload.project_id)
  .fetch_optional(&state.db_pool)
  .await
  {
    Ok(Some(row)) => row,
    Ok(None) => return HttpResponse::NotFound().body("Project not found"),
    Err(err) => return HttpResponse::InternalServerError().body(format!("Project lookup failed: {}", err)),
  };

  if !project_row.reinforcements_request {
    return HttpResponse::BadRequest().body("This project is not requesting reinforcements");
  }

  if project_row.ib_uid == session_uid {
    return HttpResponse::Forbidden().body("Project owner cannot quick-respond to own request");
  }

  let mut reinforcement_usernames: Vec<String> = project_row
    .reinforcements
    .split(',')
    .map(|item| item.trim())
    .filter(|item| !item.is_empty())
    .map(|item| item.to_string())
    .collect();

  let already_added = reinforcement_usernames
    .iter()
    .any(|name| name.eq_ignore_ascii_case(&session_username));

  if quick_action == "retreat" {
    if already_added {
      reinforcement_usernames.retain(|name| !name.eq_ignore_ascii_case(&session_username));
      let updated_reinforcements = reinforcement_usernames.join(", ");

      if let Err(err) = sqlx::query(
        "UPDATE project_profile SET reinforcements = ? WHERE id = ? LIMIT 1",
      )
      .bind(updated_reinforcements)
      .bind(payload.project_id)
      .execute(&state.db_pool)
      .await
      {
        return HttpResponse::InternalServerError().body(format!("Failed to update reinforcements: {}", err));
      }
    }
  } else if !already_added {
    reinforcement_usernames.push(session_username.clone());
    let updated_reinforcements = reinforcement_usernames.join(", ");

    if let Err(err) = sqlx::query(
      "UPDATE project_profile SET reinforcements = ? WHERE id = ? LIMIT 1",
    )
    .bind(updated_reinforcements)
    .bind(payload.project_id)
    .execute(&state.db_pool)
    .await
    {
      return HttpResponse::InternalServerError().body(format!("Failed to update reinforcements: {}", err));
    }

    let _ = state.sse_sender.send(SseEvent {
      target_uid: project_row.ib_uid,
      event_type: "reinforcement".to_string(),
      message: format!("{} responded to your reinforcements request!", session_username),
    });
  }

  let fallback_location = format!(
    "/v1/projects?ib_uid={}&ib_user={}",
    payload.ib_uid,
    payload.ib_user
  );

  let location = req
    .headers()
    .get("referer")
    .and_then(|value| value.to_str().ok())
    .filter(|value| !value.trim().is_empty())
    .unwrap_or(&fallback_location)
    .to_string();

  HttpResponse::SeeOther()
    .insert_header(("Location", location))
    .finish()
}

#[post("/v1/admin/ads/create")]
pub async fn ads_admin_create(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<AdsCreateRequest>,
) -> impl Responder {
  if !is_expected_ad_admin_identity(payload.ib_uid, &payload.ib_user)
    || !is_ad_admin_session(&req, &state).await
  {
    return HttpResponse::Forbidden().body("Forbidden");
  }

  if payload.imagepath.trim().is_empty() || payload.url.trim().is_empty() {
    return HttpResponse::BadRequest().body("imagepath and url are required");
  }

  let insert_result = sqlx::query(
    "INSERT INTO advert_image (imagepath, url, clicks, views) VALUES (?, ?, 0, 0)",
  )
  .bind(payload.imagepath.trim())
  .bind(payload.url.trim())
  .execute(&state.db_pool)
  .await;

  match insert_result {
    Ok(_) => HttpResponse::SeeOther()
      .insert_header((
        "Location",
        format!("/v1/admin/ads?ib_uid={}&ib_user={}", AD_ADMIN_UID, AD_ADMIN_USER),
      ))
      .finish(),
    Err(err) => HttpResponse::InternalServerError().body(format!("Failed to create ad: {}", err)),
  }
}

#[post("/v1/admin/ads/update")]
pub async fn ads_admin_update(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<AdsUpdateRequest>,
) -> impl Responder {
  if !is_expected_ad_admin_identity(payload.ib_uid, &payload.ib_user)
    || !is_ad_admin_session(&req, &state).await
  {
    return HttpResponse::Forbidden().body("Forbidden");
  }

  if payload.imagepath.trim().is_empty() || payload.url.trim().is_empty() {
    return HttpResponse::BadRequest().body("imagepath and url are required");
  }

  let update_result = sqlx::query(
    "UPDATE advert_image SET imagepath = ?, url = ? WHERE imageid = ? LIMIT 1",
  )
  .bind(payload.imagepath.trim())
  .bind(payload.url.trim())
  .bind(payload.imageid)
  .execute(&state.db_pool)
  .await;

  match update_result {
    Ok(_) => HttpResponse::SeeOther()
      .insert_header((
        "Location",
        format!("/v1/admin/ads?ib_uid={}&ib_user={}", AD_ADMIN_UID, AD_ADMIN_USER),
      ))
      .finish(),
    Err(err) => HttpResponse::InternalServerError().body(format!("Failed to update ad: {}", err)),
  }
}

#[post("/v1/admin/ads/delete")]
pub async fn ads_admin_delete(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<AdsDeleteRequest>,
) -> impl Responder {
  if !is_expected_ad_admin_identity(payload.ib_uid, &payload.ib_user)
    || !is_ad_admin_session(&req, &state).await
  {
    return HttpResponse::Forbidden().body("Forbidden");
  }

  let delete_result = sqlx::query("DELETE FROM advert_image WHERE imageid = ? LIMIT 1")
    .bind(payload.imageid)
    .execute(&state.db_pool)
    .await;

  match delete_result {
    Ok(_) => HttpResponse::SeeOther()
      .insert_header((
        "Location",
        format!("/v1/admin/ads?ib_uid={}&ib_user={}", AD_ADMIN_UID, AD_ADMIN_USER),
      ))
      .finish(),
    Err(err) => HttpResponse::InternalServerError().body(format!("Failed to delete ad: {}", err)),
  }
}

#[post("/v1/ads/create")]
pub async fn ads_user_create(
  req: HttpRequest,
  state: web::Data<AppState>,
  mut payload: Multipart,
) -> impl Responder {
  let Some((session_uid, session_user)) = get_session_identity(&req, &state).await else {
    return HttpResponse::Unauthorized().body("Login required");
  };

  let mut url_bytes: Vec<u8> = Vec::new();
  let mut image_bytes: Vec<u8> = Vec::new();
  let mut image_uploaded = false;

  while let Some(item) = payload.next().await {
    let mut field = match item {
      Ok(field) => field,
      Err(err) => return HttpResponse::BadRequest().body(format!("Upload read failed: {}", err)),
    };

    let field_name = field
      .content_disposition()
      .and_then(|cd| cd.get_name())
      .unwrap_or_default()
      .to_string();

    while let Some(chunk) = field.next().await {
      let data = match chunk {
        Ok(data) => data,
        Err(err) => return HttpResponse::BadRequest().body(format!("Upload chunk failed: {}", err)),
      };

      if field_name == "url" {
        if url_bytes.len() + data.len() > 4096 {
          return HttpResponse::BadRequest().body("Target URL is too large");
        }
        url_bytes.extend_from_slice(&data);
      } else if field_name == "ad_image" {
        if image_bytes.len() + data.len() > 8 * 1024 * 1024 {
          return HttpResponse::BadRequest().body("Image is too large");
        }
        image_bytes.extend_from_slice(&data);
        image_uploaded = true;
      }
    }
  }

  let target_url = String::from_utf8_lossy(&url_bytes).trim().to_string();
  if !(target_url.starts_with("https://") || target_url.starts_with("http://")) {
    return HttpResponse::BadRequest().body("Target URL must start with http:// or https://");
  }

  if !image_uploaded || image_bytes.is_empty() {
    return HttpResponse::BadRequest().body("Ad image is required");
  }

  let detected_format = match image::guess_format(&image_bytes) {
    Ok(format) => format,
    Err(_) => return HttpResponse::BadRequest().body("Unsupported image format"),
  };

  let extension = match detected_format {
    image::ImageFormat::Png => "png",
    image::ImageFormat::Jpeg => "jpg",
    image::ImageFormat::Gif => "gif",
    image::ImageFormat::WebP => "webp",
    _ => return HttpResponse::BadRequest().body("Only png, jpg, gif, webp are allowed"),
  };

  let image_dimensions = match image::load_from_memory(&image_bytes) {
    Ok(image) => image.dimensions(),
    Err(_) => return HttpResponse::BadRequest().body("Invalid image data"),
  };

  if image_dimensions != (400, 111) {
    return HttpResponse::BadRequest().body("Image must be exactly 400x111 pixels");
  }

  if let Err(err) = fs::create_dir_all("./webroot/images/advert") {
    return HttpResponse::InternalServerError().body(format!("Failed to prepare advert directory: {}", err));
  }

  let file_name = format!("ad_{}_{}.{}", session_uid, Uuid::new_v4(), extension);
  let relative_image_path = format!("/images/advert/{}", file_name);
  let disk_path = format!("./webroot{}", relative_image_path);

  if let Err(err) = fs::write(&disk_path, &image_bytes) {
    return HttpResponse::InternalServerError().body(format!("Failed to save image: {}", err));
  }

  let existing_ad_id = sqlx::query_scalar::<_, i64>(
    "SELECT imageid FROM advert_image WHERE owner_uid = ? AND payment_status = 'paid' AND is_active = FALSE LIMIT 1"
  )
  .bind(session_uid)
  .fetch_optional(&state.db_pool)
  .await
  .unwrap_or(None);

  if let Some(imageid) = existing_ad_id {
    let update_result = sqlx::query(
      "UPDATE advert_image SET imagepath = ?, url = ?, clicks = 0, views = 0, is_active = TRUE WHERE imageid = ? LIMIT 1"
    )
    .bind(&relative_image_path)
    .bind(&target_url)
    .bind(imageid)
    .execute(&state.db_pool)
    .await;

    match update_result {
      Ok(_) => HttpResponse::SeeOther()
        .insert_header(("Location", "/v1/ads"))
        .finish(),
      Err(err) => HttpResponse::InternalServerError().body(format!("Failed to recycle existing paid ad: {}", err)),
    }
  } else {
    let insert_result = sqlx::query(
      "INSERT INTO advert_image (imagepath, url, owner_uid, owner_username, payment_status, clicks, views, is_active) VALUES (?, ?, ?, ?, 'pending', 0, 0, TRUE)",
    )
    .bind(&relative_image_path)
    .bind(&target_url)
    .bind(session_uid)
    .bind(&session_user)
    .execute(&state.db_pool)
    .await;

    let imageid = match insert_result {
      Ok(result) => result.last_insert_id() as i64,
      Err(err) => return HttpResponse::InternalServerError().body(format!("Failed to create ad: {}", err)),
    };

    match paypal_create_subscription(imageid).await {
      Ok(approval_url) => HttpResponse::SeeOther()
        .insert_header(("Location", approval_url))
        .finish(),
      Err(err) => HttpResponse::InternalServerError().body(format!("Ad created but PayPal subscription failed: {}", err)),
    }
  }
}

#[post("/v1/ads/pay/{imageid}")]
pub async fn ads_user_pay(
  req: HttpRequest,
  state: web::Data<AppState>,
  imageid: web::Path<i64>,
) -> impl Responder {
  let Some((session_uid, _)) = get_session_identity(&req, &state).await else {
    return HttpResponse::Unauthorized().body("Login required");
  };

  let imageid = imageid.into_inner();
  let ownership_count = sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM advert_image WHERE imageid = ? AND owner_uid = ?",
  )
  .bind(imageid)
  .bind(session_uid)
  .fetch_one(&state.db_pool)
  .await;

  match ownership_count {
    Ok(count) if count > 0 => {}
    Ok(_) => return HttpResponse::Forbidden().body("Not your ad"),
    Err(err) => return HttpResponse::InternalServerError().body(format!("Ownership check failed: {}", err)),
  }

  match paypal_create_subscription(imageid).await {
    Ok(approval_url) => HttpResponse::SeeOther()
      .insert_header(("Location", approval_url))
      .finish(),
    Err(err) => HttpResponse::InternalServerError().body(format!("PayPal subscription failed: {}", err)),
  }
}

#[get("/v1/ads/paypal/return")]
pub async fn ads_paypal_return(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<PayPalReturnQuery>,
) -> impl Responder {
  let Some((session_uid, _)) = get_session_identity(&req, &state).await else {
    return HttpResponse::Unauthorized().body("Login required");
  };

  let access_token = match paypal_access_token().await {
    Ok(token) => token,
    Err(err) => return HttpResponse::InternalServerError().body(err),
  };

  let subscription_id = query
    .subscription_id
    .clone()
    .or_else(|| query.token.clone())
    .or_else(|| query.ba_token.clone())
    .unwrap_or_default();

  if subscription_id.trim().is_empty() {
    return HttpResponse::BadRequest().body("Missing PayPal subscription identifier");
  }

  let subscription_endpoint = format!(
    "{}/v1/billing/subscriptions/{}",
    paypal_base_url().trim_end_matches('/'),
    subscription_id
  );

  let subscription_response = match reqwest::Client::new()
    .get(subscription_endpoint)
    .bearer_auth(&access_token)
    .send()
    .await
  {
    Ok(response) => response,
    Err(err) => return HttpResponse::InternalServerError().body(format!("PayPal subscription lookup failed: {}", err)),
  };

  if !subscription_response.status().is_success() {
    let body = subscription_response.text().await.unwrap_or_default();
    return HttpResponse::InternalServerError().body(format!("PayPal subscription lookup rejected: {}", body));
  }

  let subscription_json: Value = match subscription_response.json().await {
    Ok(json) => json,
    Err(err) => return HttpResponse::InternalServerError().body(format!("PayPal subscription lookup parse failed: {}", err)),
  };

  let subscription_status = subscription_json
    .get("status")
    .and_then(Value::as_str)
    .unwrap_or_default()
    .to_string();

  let custom_id = subscription_json
    .get("custom_id")
    .and_then(Value::as_str)
    .unwrap_or_default();

  let imageid = match custom_id.parse::<i64>() {
    Ok(id) => id,
    Err(_) => return HttpResponse::BadRequest().body("Invalid PayPal custom_id"),
  };

  let ownership_count = match sqlx::query_scalar::<_, i64>(
    "SELECT COUNT(*) FROM advert_image WHERE imageid = ? AND owner_uid = ?",
  )
  .bind(imageid)
  .bind(session_uid)
  .fetch_one(&state.db_pool)
  .await
  {
    Ok(count) => count,
    Err(err) => return HttpResponse::InternalServerError().body(format!("Ownership check failed: {}", err)),
  };

  if ownership_count == 0 {
    return HttpResponse::Forbidden().body("Not your ad");
  }

  let local_payment_status = if subscription_status == "ACTIVE" {
    "paid"
  } else {
    "pending"
  };

  let update_result = sqlx::query(
    "UPDATE advert_image SET payment_status = ?, paypal_order_id = ? WHERE imageid = ? AND owner_uid = ? LIMIT 1",
  )
  .bind(local_payment_status)
  .bind(&subscription_id)
  .bind(imageid)
  .bind(session_uid)
  .execute(&state.db_pool)
  .await;

  match update_result {
    Ok(_) => HttpResponse::SeeOther()
      .insert_header(("Location", "/v1/ads"))
      .finish(),
    Err(err) => HttpResponse::InternalServerError().body(format!("Failed to mark ad as paid: {}", err)),
  }
}

#[get("/v1/ads/paypal/cancel")]
pub async fn ads_paypal_cancel(
  req: HttpRequest,
  state: web::Data<AppState>,
) -> impl Responder {
  if get_session_identity(&req, &state).await.is_none() {
    return HttpResponse::Unauthorized().body("Login required");
  }

  HttpResponse::SeeOther()
    .insert_header(("Location", "/v1/ads"))
    .finish()
}

#[post("/v1/ads/update")]
pub async fn ads_user_update(
  req: HttpRequest,
  state: web::Data<AppState>,
  mut payload: Multipart,
) -> impl Responder {
  let Some((session_uid, _)) = get_session_identity(&req, &state).await else {
    return HttpResponse::Unauthorized().body("Login required");
  };

  let mut url_bytes: Vec<u8> = Vec::new();
  let mut imageid_bytes: Vec<u8> = Vec::new();
  let mut image_bytes: Vec<u8> = Vec::new();
  let mut image_uploaded = false;

  while let Some(item) = payload.next().await {
    let mut field = match item {
      Ok(field) => field,
      Err(err) => return HttpResponse::BadRequest().body(format!("Upload read failed: {}", err)),
    };

    let field_name = field
      .content_disposition()
      .and_then(|cd| cd.get_name())
      .unwrap_or_default()
      .to_string();

    while let Some(chunk) = field.next().await {
      let data = match chunk {
        Ok(data) => data,
        Err(err) => return HttpResponse::BadRequest().body(format!("Upload chunk failed: {}", err)),
      };

      if field_name == "url" {
        url_bytes.extend_from_slice(&data);
      } else if field_name == "imageid" {
        imageid_bytes.extend_from_slice(&data);
      } else if field_name == "ad_image" {
        if !data.is_empty() {
          image_uploaded = true;
          image_bytes.extend_from_slice(&data);
        }
      }
    }
  }

  let target_url = String::from_utf8_lossy(&url_bytes).trim().to_string();
  if !(target_url.starts_with("https://") || target_url.starts_with("http://")) {
    return HttpResponse::BadRequest().body("Target URL must start with http:// or https://");
  }

  let imageid_str = String::from_utf8_lossy(&imageid_bytes).trim().to_string();
  let imageid: i64 = match imageid_str.parse() {
    Ok(val) => val,
    Err(_) => return HttpResponse::BadRequest().body("Invalid imageid"),
  };

  if image_uploaded && !image_bytes.is_empty() {
    let detected_format = match image::guess_format(&image_bytes) {
      Ok(format) => format,
      Err(_) => return HttpResponse::BadRequest().body("Unsupported image format"),
    };

    let extension = match detected_format {
      image::ImageFormat::Png => "png",
      image::ImageFormat::Jpeg => "jpg",
      image::ImageFormat::Gif => "gif",
      image::ImageFormat::WebP => "webp",
      _ => return HttpResponse::BadRequest().body("Only png, jpg, gif, webp are allowed"),
    };

    let image_dimensions = match image::load_from_memory(&image_bytes) {
      Ok(image) => image.dimensions(),
      Err(_) => return HttpResponse::BadRequest().body("Invalid image data"),
    };

    if image_dimensions != (400, 111) {
      return HttpResponse::BadRequest().body("Image must be exactly 400x111 pixels");
    }

    if let Err(err) = fs::create_dir_all("./webroot/images/advert") {
      return HttpResponse::InternalServerError().body(format!("Failed to prepare advert directory: {}", err));
    }

    let file_name = format!("ad_{}_{}.{}", session_uid, Uuid::new_v4(), extension);
    let relative_image_path = format!("/images/advert/{}", file_name);
    let disk_path = format!("./webroot{}", relative_image_path);

    if let Err(err) = fs::write(&disk_path, &image_bytes) {
      return HttpResponse::InternalServerError().body(format!("Failed to save image: {}", err));
    }

    let update_result = sqlx::query(
      "UPDATE advert_image SET imagepath = ?, url = ? WHERE imageid = ? AND owner_uid = ? LIMIT 1",
    )
    .bind(&relative_image_path)
    .bind(&target_url)
    .bind(imageid)
    .bind(session_uid)
    .execute(&state.db_pool)
    .await;

    match update_result {
      Ok(_) => HttpResponse::SeeOther().insert_header(("Location", "/v1/ads")).finish(),
      Err(err) => HttpResponse::InternalServerError().body(format!("Failed to update ad with image: {}", err)),
    }
  } else {
    let update_result = sqlx::query(
      "UPDATE advert_image SET url = ? WHERE imageid = ? AND owner_uid = ? LIMIT 1",
    )
    .bind(&target_url)
    .bind(imageid)
    .bind(session_uid)
    .execute(&state.db_pool)
    .await;

    match update_result {
      Ok(_) => HttpResponse::SeeOther().insert_header(("Location", "/v1/ads")).finish(),
      Err(err) => HttpResponse::InternalServerError().body(format!("Failed to update ad URL: {}", err)),
    }
  }
}

#[post("/v1/ads/delete")]
pub async fn ads_user_delete(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Form<AdsUserDeleteRequest>,
) -> impl Responder {
  let Some((session_uid, _)) = get_session_identity(&req, &state).await else {
    return HttpResponse::Unauthorized().body("Login required");
  };

  let imagepath = sqlx::query_scalar::<_, String>(
    "SELECT imagepath FROM advert_image WHERE imageid = ? AND owner_uid = ? LIMIT 1",
  )
  .bind(payload.imageid)
  .bind(session_uid)
  .fetch_optional(&state.db_pool)
  .await;

  let imagepath = match imagepath {
    Ok(Some(path)) => path,
    Ok(None) => return HttpResponse::Forbidden().body("Not your ad"),
    Err(err) => return HttpResponse::InternalServerError().body(format!("Ad lookup failed: {}", err)),
  };

  let delete_result = sqlx::query("DELETE FROM advert_image WHERE imageid = ? AND owner_uid = ? LIMIT 1")
    .bind(payload.imageid)
    .bind(session_uid)
    .execute(&state.db_pool)
    .await;

  if let Err(err) = delete_result {
    return HttpResponse::InternalServerError().body(format!("Failed to delete ad: {}", err));
  }

  if imagepath.starts_with("/images/advert/") {
    let disk_path = format!("./webroot{}", imagepath);
    let _ = fs::remove_file(disk_path);
  }

  HttpResponse::SeeOther()
    .insert_header(("Location", "/v1/ads"))
    .finish()
}

#[get("/api/v1/inbox/contacts")]
pub async fn get_inbox_contacts_page(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<InboxContactsPageQuery>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => {
      return HttpResponse::Unauthorized().json(json!({
        "error": "Login required"
      }));
    }
  };

  if session_uid != query.ib_uid {
    return HttpResponse::Forbidden().json(json!({
      "error": "Session mismatch"
    }));
  }

  let offset = query.offset.unwrap_or(0).max(0) as usize;
  let limit = query.limit.unwrap_or(20).clamp(1, 50) as usize;

  match load_inbox_contacts(&state, query.ib_uid, &query.ib_user).await {
    Ok(all_contacts) => {
      let total_contacts = all_contacts.len();
      let start = offset.min(total_contacts);
      let end = start.saturating_add(limit).min(total_contacts);
      let selected_contacts = &all_contacts[start..end];

      let contacts_html = if selected_contacts.is_empty() {
        String::new()
      } else {
        render_inbox_contacts_html(selected_contacts)
      };

      HttpResponse::Ok().json(InboxContactsPageResponse {
        contacts_html,
        has_more: end < total_contacts,
        next_offset: end,
      })
    }
    Err(err) => HttpResponse::InternalServerError().json(json!({
      "error": err
    })),
  }
}

#[post("/v1/dm/send")]
pub async fn send_direct_message(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: Either<web::Json<DMMessageRequest>, web::Form<DMMessageRequest>>,
) -> impl Responder {
  let payload = match payload {
    Either::Left(json_payload) => json_payload.into_inner(),
    Either::Right(form_payload) => form_payload.into_inner(),
  };

  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => {
      return HttpResponse::Unauthorized().json(DMSendResponse {
        success: false,
        message: "Login required".to_string(),
      });
    }
  };

  let target_user = payload.target_user.trim();
  let message = payload.message.trim();

  if target_user.is_empty() || message.is_empty() {
    return HttpResponse::BadRequest().json(DMSendResponse {
      success: false,
      message: "Target user and message are required".to_string(),
    });
  }

  let (target_uid, _) = match lookup_user_by_username(&state, target_user).await {
    Ok(Some(found)) => found,
    Ok(None) => {
      return HttpResponse::NotFound().json(DMSendResponse {
        success: false,
        message: "Target user not found".to_string(),
      });
    }
    Err(err) => {
      return HttpResponse::InternalServerError().json(DMSendResponse {
        success: false,
        message: format!("Lookup failed: {}", err),
      });
    }
  };

  if target_uid == session_uid {
    return HttpResponse::BadRequest().json(DMSendResponse {
      success: false,
      message: "Cannot send a direct message to yourself".to_string(),
    });
  }

  if crate::db::is_blocked(&state, Some(session_uid), Some(target_uid)).await {
    return HttpResponse::Forbidden().json(DMSendResponse {
      success: false,
      message: "You cannot send messages to this user".to_string(),
    });
  }

  let stored_message = match encode_dm_message_for_storage(message) {
    Ok(em) => em,
    Err(e) => return HttpResponse::InternalServerError().json(DMSendResponse {
        success: false,
        message: e,
    }),
  };

  let insert_result = sqlx::query(
    "INSERT INTO dm (sender_uid, recipient_uid, message) VALUES (?, ?, ?)",
  )
  .bind(session_uid)
  .bind(target_uid)
  .bind(stored_message)
  .execute(&state.db_pool)
  .await;

  let wants_json = req
    .headers()
    .get("accept")
    .and_then(|value| value.to_str().ok())
    .map(|value| value.contains("application/json"))
    .unwrap_or(false);

  match insert_result {
    Ok(_) => {
      let _ = state.sse_sender.send(SseEvent {
        target_uid,
        event_type: "dm".to_string(),
        message: "You have a new direct message.".to_string(),
      });

      let current_username = match sqlx::query_as::<_, SessionUserRow>(
        "SELECT username FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1",
      )
      .bind(session_uid.to_string())
      .fetch_optional(&state.db_pool)
      .await
      {
        Ok(Some(row)) if !row.username.trim().is_empty() => row.username,
        _ => req.cookie("ib_user").map(|cookie| cookie.value().to_string()).unwrap_or_default(),
      };

      let subscriptions = sqlx::query_as::<_, PushSubscriptionRow>(
        "SELECT endpoint, p256dh, auth FROM push_subscriptions WHERE ib_uid = ?"
      )
      .bind(target_uid)
      .fetch_all(&state.db_pool)
      .await
      .unwrap_or_default();

      if !subscriptions.is_empty() {
        let vapid_private = state.vapid_private_key.clone();
        let payload = json!({
          "title": "New Message",
          "body": format!("New direct message from @{}", current_username),
          "url": format!("https://{}/v1/inbox?target_user={}", DOMAIN, url_encode_component(&current_username)),
        }).to_string();

        tokio::spawn(async move {
          use web_push::{IsahcWebPushClient, WebPushClient, WebPushMessageBuilder, SubscriptionInfo, VapidSignatureBuilder, ContentEncoding};

          let client = match IsahcWebPushClient::new() {
            Ok(c) => c,
            Err(e) => {
              eprintln!("Failed to create IsahcWebPushClient: {}", e);
              return;
            }
          };

          for sub in subscriptions {
            let subscription_info = SubscriptionInfo::new(
              &sub.endpoint,
              &sub.p256dh,
              &sub.auth,
            );

            let mut sig_builder = match VapidSignatureBuilder::from_base64(&vapid_private, &subscription_info) {
              Ok(builder) => builder,
              Err(e) => {
                eprintln!("Failed to create VapidSignatureBuilder: {}", e);
                continue;
              }
            };
            
            sig_builder.add_claim("sub", json!(format!("mailto:admin@{}", DOMAIN)));
            let signature = match sig_builder.build() {
              Ok(sig) => sig,
              Err(e) => {
                eprintln!("Failed to build VAPID signature: {}", e);
                continue;
              }
            };

            let mut builder = WebPushMessageBuilder::new(&subscription_info);
            builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_bytes());
            builder.set_vapid_signature(signature);

            let message = match builder.build() {
              Ok(msg) => msg,
              Err(e) => {
                eprintln!("Failed to build web push message: {}", e);
                continue;
              }
            };

            if let Err(e) = client.send(message).await {
              eprintln!("Failed to send web push: {}", e);
            }
          }
        });
      }

      if wants_json {
        HttpResponse::Ok().json(DMSendResponse {
          success: true,
          message: "Message sent".to_string(),
        })
      } else {
        HttpResponse::SeeOther()
          .insert_header((
            "Location",
            format!(
              "/v1/inbox?ib_uid={}&ib_user={}&target_user={}",
              session_uid,
              current_username,
              payload.target_user
            ),
          ))
          .finish()
      }
    }
    Err(err) => {
      if wants_json {
        HttpResponse::InternalServerError().json(DMSendResponse {
          success: false,
          message: format!("Failed to send message: {}", err),
        })
      } else {
        HttpResponse::InternalServerError().body(format!("Failed to send message: {}", err))
      }
    }
  }
}

#[get("/v1/ad/click/{imageid}")]
pub async fn ad_click(
  state: web::Data<AppState>,
  path: web::Path<i64>,
) -> impl Responder {
  let imageid = path.into_inner();

  let ad_row = sqlx::query_as::<_, AdvertImageRow>(
      "SELECT imageid, imagepath, url FROM advert_image WHERE imageid = ? AND payment_status = 'paid' LIMIT 1"
    )
    .bind(imageid)
    .fetch_optional(&state.db_pool)
    .await;

  let Some(ad_row) = ad_row.ok().flatten() else {
    return HttpResponse::SeeOther()
      .insert_header(("Location", "https://is-by.pro/advertise.html"))
      .finish();
  };

  let _ = sqlx::query(
    "UPDATE advert_image SET clicks = COALESCE(clicks, 0) + 1 WHERE imageid = ? LIMIT 1",
  )
  .bind(imageid)
  .execute(&state.db_pool)
  .await;

  HttpResponse::SeeOther()
    .insert_header(("Location", ad_row.url))
    .finish()
}

#[get("/v1/dm/messages")]
pub async fn direct_messages(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<DMMessagesRequest>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => {
      return HttpResponse::Unauthorized().json(DMMessagesResponse {
        success: false,
        messages: Vec::new(),
        has_more: false,
      });
    }
  };

  let target_user = query.target_user.trim();
  if target_user.is_empty() {
    return HttpResponse::BadRequest().json(DMMessagesResponse {
      success: false,
      messages: Vec::new(),
      has_more: false,
    });
  }

  let (target_uid, _) = match lookup_user_by_username(&state, target_user).await {
    Ok(Some(found)) => found,
    Ok(None) => {
      return HttpResponse::NotFound().json(DMMessagesResponse {
        success: false,
        messages: Vec::new(),
        has_more: false,
      });
    }
    Err(_) => {
      return HttpResponse::InternalServerError().json(DMMessagesResponse {
        success: false,
        messages: Vec::new(),
        has_more: false,
      });
    }
  };

  let limit = 21; // Fetch one extra to check for more
  let rows_result = if let Some(before_id) = query.before_id {
    sqlx::query_as::<_, DMMessageRow>(
      "SELECT dm.id, dm.sender_uid, CAST(COALESCE(CONVERT(sender.username USING utf8mb4), CAST(dm.sender_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS sender_username, CAST(COALESCE(CONVERT(recipient.username USING utf8mb4), CAST(dm.recipient_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS recipient_username, dm.message, DATE_FORMAT(dm.created_at, '%Y-%m-%d %H:%i:%s') AS created_at FROM dm AS dm LEFT JOIN user AS sender ON CONVERT(sender.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(dm.sender_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci LEFT JOIN user AS recipient ON CONVERT(recipient.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(dm.recipient_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE ((dm.sender_uid = ? AND dm.recipient_uid = ?) OR (dm.sender_uid = ? AND dm.recipient_uid = ?)) AND dm.id < ? ORDER BY dm.id DESC LIMIT ?",
    )
    .bind(session_uid)
    .bind(target_uid)
    .bind(target_uid)
    .bind(session_uid)
    .bind(before_id)
    .bind(limit)
    .fetch_all(&state.db_pool)
    .await
  } else {
    sqlx::query_as::<_, DMMessageRow>(
      "SELECT dm.id, dm.sender_uid, CAST(COALESCE(CONVERT(sender.username USING utf8mb4), CAST(dm.sender_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS sender_username, CAST(COALESCE(CONVERT(recipient.username USING utf8mb4), CAST(dm.recipient_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS recipient_username, dm.message, DATE_FORMAT(dm.created_at, '%Y-%m-%d %H:%i:%s') AS created_at FROM dm AS dm LEFT JOIN user AS sender ON CONVERT(sender.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(dm.sender_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci LEFT JOIN user AS recipient ON CONVERT(recipient.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(dm.recipient_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE (dm.sender_uid = ? AND dm.recipient_uid = ?) OR (dm.sender_uid = ? AND dm.recipient_uid = ?) ORDER BY dm.id DESC LIMIT ?",
    )
    .bind(session_uid)
    .bind(target_uid)
    .bind(target_uid)
    .bind(session_uid)
    .bind(limit)
    .fetch_all(&state.db_pool)
    .await
  };

  let mut rows = match rows_result {
    Ok(rows) => rows,
    Err(_) => {
      return HttpResponse::InternalServerError().json(DMMessagesResponse {
        success: false,
        messages: Vec::new(),
        has_more: false,
      });
    }
  };

  let has_more = rows.len() == limit as usize;
  if has_more {
    rows.truncate(limit as usize - 1);
  }

  if query.before_id.is_none() {
    let _ = sqlx::query(
      "UPDATE dm SET read_at = NOW() WHERE sender_uid = ? AND recipient_uid = ? AND read_at IS NULL",
    )
    .bind(target_uid)
    .bind(session_uid)
    .execute(&state.db_pool)
    .await;
  }

  let mut messages = rows
    .into_iter()
    .map(|row| {
      let decrypted_message = decode_dm_message_from_storage(&row.message)
        .unwrap_or_else(|_| "[DECRYPTION FAILED]".to_string());
      DMMessageResponseItem {
        id: row.id,
        sender_user: row.sender_username,
        recipient_user: row.recipient_username,
        message: decrypted_message,
        timestamp: row.created_at,
        is_mine: row.sender_uid == session_uid,
      }
    })
    .collect::<Vec<DMMessageResponseItem>>();

  messages.reverse();

  HttpResponse::Ok().json(DMMessagesResponse {
    success: true,
    messages,
    has_more,
  })
}

#[get("/v1/dm/unreadcount")]
pub async fn direct_message_unread_count(
  req: HttpRequest,
  state: web::Data<AppState>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => {
      return HttpResponse::Unauthorized().json(DMUnreadCountResponse {
        success: false,
        unread_count: 0,
      });
    }
  };

  let row = sqlx::query_as::<_, DMUnreadCountRow>(
      "SELECT COUNT(*) AS unread_count FROM dm WHERE recipient_uid = ? AND read_at IS NULL"
    )
    .bind(session_uid)
    .fetch_one(&state.db_pool)
    .await;

  match row {
    Ok(row) => HttpResponse::Ok().json(DMUnreadCountResponse {
      success: true,
      unread_count: row.unread_count,
    }),
    Err(_) => HttpResponse::InternalServerError().json(DMUnreadCountResponse {
      success: false,
      unread_count: 0,
    }),
  }
}
pub fn github_authorize_redirect_uri(req: &HttpRequest) -> String {
  format!("https://{}{}", req.connection_info().host(), "/v1/auth/github/callback")
}
pub fn github_callback_redirect_uri(req: &HttpRequest) -> String {
  format!("https://{}{}", req.connection_info().host(), req.path())
}
pub async fn github_auth_start_impl(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
  let oauth_state: String = rand::thread_rng()
    .sample_iter(&Alphanumeric)
    .take(32)
    .map(char::from)
    .collect();

  let redirect_uri = github_authorize_redirect_uri(&req);

  let url = format!(
    "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=read:user%20user:email&state={}",
    state.github_client_id,
    redirect_uri,
    oauth_state
  );

  HttpResponse::Found()
    .insert_header(("Location", url))
    .cookie(
      Cookie::build("gh_oauth_state", oauth_state)
        .path("/")
        .http_only(true)
        .secure(true)
        .finish(),
    )
    .finish()
}

#[get("/v1/auth/github")]
pub async fn github_auth_start_v1(req: HttpRequest, state: web::Data<AppState>) -> impl Responder {
  github_auth_start_impl(req, state).await
}
pub async fn github_auth_callback_impl(
  req: HttpRequest,
  query: web::Query<GithubCallback>,
  state: web::Data<AppState>,
) -> HttpResponse {
  let redirect_uri = github_callback_redirect_uri(&req);

  let cookie_state = match req.cookie("gh_oauth_state") {
    Some(c) => c.value().to_string(),
    None => return HttpResponse::BadRequest().body("Missing oauth state cookie"),
  };

  if cookie_state != query.state {
    return HttpResponse::BadRequest().body("Invalid oauth state");
  }

  let client = reqwest::Client::new();
  let token_res = match client
    .post("https://github.com/login/oauth/access_token")
    .header("Accept", "application/json")
    .form(&[
      ("client_id", state.github_client_id.as_str()),
      ("client_secret", state.github_client_secret.as_str()),
      ("code", query.code.as_str()),
      ("redirect_uri", redirect_uri.as_str()),
    ])
    .send()
    .await
  {
    Ok(r) => r,
    Err(_) => return HttpResponse::InternalServerError().body("GitHub token request failed"),
  };

  let token_status = token_res.status();
  let token_body = match token_res.text().await {
    Ok(body) => body,
    Err(_) => return HttpResponse::InternalServerError().body("Failed to read token response"),
  };

  let token_data = if token_status.is_success() {
    if let Ok(err_data) = serde_json::from_str::<GithubTokenErrorResponse>(&token_body) {
      return HttpResponse::BadRequest()
        .cookie(remove_cookie("gh_oauth_state"))
        .body(format!(
          "GitHub login code expired or was already used. Start login again. {} ({})",
          err_data.error,
          err_data.error_description.unwrap_or_else(|| "no description".to_string())
        ));
    }

    match serde_json::from_str::<GithubTokenResponse>(&token_body) {
      Ok(d) => d,
      Err(_) => {
        return HttpResponse::InternalServerError()
          .cookie(remove_cookie("gh_oauth_state"))
          .body(format!(
            "Invalid token response body: {}",
            token_body
          ))
      }
    }
  } else {
    if let Ok(err_data) = serde_json::from_str::<GithubTokenErrorResponse>(&token_body) {
      return HttpResponse::BadRequest()
        .cookie(remove_cookie("gh_oauth_state"))
        .body(format!(
          "GitHub token error: {} ({})",
          err_data.error,
          err_data.error_description.unwrap_or_else(|| "no description".to_string())
        ));
    }

    return HttpResponse::BadRequest()
      .cookie(remove_cookie("gh_oauth_state"))
      .body(format!(
        "GitHub token exchange failed (status {}): {}",
        token_status,
        token_body
      ));
  };

  let user_res = match client
    .get("https://api.github.com/user")
    .header("User-Agent", "is-by_pro")
    .bearer_auth(token_data.access_token)
    .send()
    .await
  {
    Ok(r) => r,
    Err(_) => return HttpResponse::InternalServerError().body("GitHub user request failed"),
  };

  let user = match user_res.json::<GithubUser>().await {
    Ok(u) => u,
    Err(_) => return HttpResponse::InternalServerError().body("Invalid user response"),
  };

  if let Err(err) = ensure_legacy_user_from_github(&state, user.id, &user.login).await {
    return HttpResponse::InternalServerError()
      .body(format!("Failed to persist GitHub user: {}", err));
  }

  match render_profile_html(&state, user.id as i64, &user.login, Some(user.id as i64)).await {
    Ok(_) => HttpResponse::SeeOther()
      .insert_header(("Location", format!("/v1/profile/{}", user.login)))
      .cookie(
        Cookie::build("ib_uid", user.id.to_string())
          .path("/")
          .http_only(true)
          .secure(true)
          .finish(),
      )
      .cookie(
        Cookie::build("ib_user", user.login.clone())
          .path("/")
          .http_only(true)
          .secure(true)
          .finish(),
      )
      .cookie(remove_cookie("gh_oauth_state"))
      .finish(),
    Err(err) => HttpResponse::InternalServerError()
      .cookie(remove_cookie("gh_oauth_state"))
      .body(err),
  }
}

#[get("/v1/auth/github/callback")]
pub async fn github_auth_callback_v1(
  req: HttpRequest,
  query: web::Query<GithubCallback>,
  state: web::Data<AppState>,
) -> impl Responder {
  github_auth_callback_impl(req, query, state).await
}

#[post("/v1/ads/paypal/webhook")]
pub async fn ads_paypal_webhook(
  state: web::Data<AppState>,
  body: web::Bytes,
) -> impl Responder {
  let json: serde_json::Value = match serde_json::from_slice(&body) {
    Ok(json) => json,
    Err(e) => {
      eprintln!("PayPal webhook JSON parse error: {}", e);
      return HttpResponse::BadRequest().body("Invalid JSON");
    }
  };

  let event_type = json.get("event_type").and_then(|v| v.as_str()).unwrap_or("");
  if event_type == "BILLING.SUBSCRIPTION.CANCELLED" 
     || event_type == "BILLING.SUBSCRIPTION.EXPIRED" 
     || event_type == "BILLING.SUBSCRIPTION.SUSPENDED" {
    
    if let Some(resource) = json.get("resource") {
      if let Some(subscription_id) = resource.get("id").and_then(|v| v.as_str()) {
        let update_result = sqlx::query(
          "UPDATE advert_image SET payment_status = 'expired' WHERE paypal_order_id = ? AND payment_status IN ('paid')"
        )
        .bind(subscription_id)
        .execute(&state.db_pool)
        .await;

        if let Err(e) = update_result {
          eprintln!("Failed to update expired subscription in db: {}", e);
        }
      }
    }
  }

  HttpResponse::Ok().finish()
}

#[get("/v1/push/public-key")]
pub async fn get_vapid_public_key(state: web::Data<AppState>) -> impl Responder {
  HttpResponse::Ok().body(state.vapid_public_key.clone())
}

#[post("/v1/push/subscribe")]
pub async fn subscribe_push(
  req: HttpRequest,
  state: web::Data<AppState>,
  payload: web::Json<PushSubscriptionRequest>,
) -> impl Responder {
  let session_uid = match get_session_uid(&req) {
    Some(uid) => uid,
    None => return HttpResponse::Unauthorized().json(json!({"success": false, "message": "Login required"})),
  };

  let sub = payload.into_inner();
  
  if sub.endpoint.is_empty() || sub.keys.p256dh.is_empty() || sub.keys.auth.is_empty() {
    return HttpResponse::BadRequest().json(json!({"success": false, "message": "Invalid subscription payload"}));
  }

  // Insert or update
  let result = sqlx::query(
    "INSERT INTO push_subscriptions (ib_uid, endpoint, p256dh, auth, created_at) VALUES (?, ?, ?, ?, NOW()) ON DUPLICATE KEY UPDATE p256dh = VALUES(p256dh), auth = VALUES(auth), created_at = NOW()"
  )
  .bind(session_uid)
  .bind(&sub.endpoint)
  .bind(&sub.keys.p256dh)
  .bind(&sub.keys.auth)
  .execute(&state.db_pool)
  .await;

  match result {
    Ok(_) => HttpResponse::Ok().json(json!({"success": true})),
    Err(e) => {
      eprintln!("Failed to save push subscription: {}", e);
      HttpResponse::InternalServerError().json(json!({"success": false, "message": "Database error"}))
    }
  }
}

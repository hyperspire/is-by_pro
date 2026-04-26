use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use crate::{models::*, utils::*, auth::*, render::*};
use is_by_pro::COPYRIGHT;
use crate::{DOMAIN, AD_ADMIN_UID, AD_ADMIN_USER};

#[get("/v1/profile/{ib_user}")]
pub async fn view_profile(
  req: HttpRequest,
  path: web::Path<String>,
  state: web::Data<AppState>,
) -> impl Responder {
  let ib_user = path.into_inner();

  if let Some(stripped) = ib_user.strip_prefix('@') {
    return HttpResponse::PermanentRedirect()
      .insert_header(("Location", format!("/v1/profile/{}", stripped)))
      .finish();
  }

  let normalized = ib_user.clone();

  // 1) Accept numeric uid directly in path.
  let mut resolved_uid = normalized.parse::<i64>().ok();

  // 2) Resolve from profile github handle.
  if resolved_uid.is_none() {
    let lookup = sqlx::query_as::<_, ProfileLookupRow>(
      "SELECT ib_uid FROM pro WHERE LOWER(github) = LOWER(?) LIMIT 1",
    )
    .bind(&normalized)
    .fetch_optional(&state.db_pool)
    .await;

    match lookup {
      Ok(Some(row)) => {
        if let Ok(parsed) = row.ib_uid.parse::<i64>() {
          resolved_uid = Some(parsed);
        }
      }
      Ok(None) => {}
      Err(err) => {
        return HttpResponse::InternalServerError().body(format!("Profile lookup failed: {}", err));
      }
    }
  }

  // CREATE TABLE user (ib_uid varchar(64) PRIMARY KEY, username varchar(255));
  // 3) Fallback by username in legacy user table.
  if resolved_uid.is_none() {
    let user_lookup = sqlx::query_as::<_, UsernameLookupRow>(
      "SELECT ib_uid FROM user WHERE LOWER(username) = LOWER(?) LIMIT 1",
    )
    .bind(&normalized)
    .fetch_optional(&state.db_pool)
    .await;

    match user_lookup {
      Ok(Some(row)) => {
        if let Ok(parsed) = row.ib_uid.parse::<i64>() {
          resolved_uid = Some(parsed);
        }
      }
      Ok(None) => {}
      Err(err) => {
        return HttpResponse::InternalServerError().body(format!("Username lookup failed: {}", err));
      }
    }
  }

  let ib_uid = match resolved_uid {
    Some(uid) => uid,
    None => {
      return HttpResponse::NotFound()
        .content_type("text/html; charset=utf-8")
        .body(format!(
          r#"<!DOCTYPE html><html lang="en-US"><head><meta charset="UTF-8"><title>Profile Not Found</title></head><body><p>Profile not found: {}</p></body></html>"#,
          escape_html(&ib_user)
        ));
    }
  };

  if is_mobile_device(&req) {
    match render_profile_mobile_html(&state, ib_uid, &ib_user, get_session_uid(&req)).await {
      Ok(html) => HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html),
      Err(err) => HttpResponse::InternalServerError().body(err),
    }
  } else {
    match render_profile_html(&state, ib_uid, &ib_user, get_session_uid(&req)).await {
      Ok(html) => HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html),
      Err(err) => HttpResponse::InternalServerError().body(err),
    }
  }
}
#[get("/v1/searchusers")]
pub async fn search_users(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<SearchUsersRequest>,
) -> impl Responder {
  let session_uid = get_session_uid(&req);
  let html_result = if is_mobile_device(&req) {
    render_search_users_mobile_html(&state, query.ib_uid, &query.ib_user, &query.query, session_uid).await
  } else {
    render_search_users_html(&state, query.ib_uid, &query.ib_user, &query.query, session_uid).await
  };
  match html_result {
    Ok(html) => HttpResponse::Ok()
      .content_type("text/html; charset=utf-8")
      .body(html),
    Err(err) => HttpResponse::InternalServerError().body(err),
  }
}
#[get("/v1/searchposts")]
pub async fn search_posts(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<SearchPostsRequest>,
) -> impl Responder {
  let session_uid = get_session_uid(&req);
  let html_result = if is_mobile_device(&req) {
    render_search_posts_mobile_html(&state, query.ib_uid, &query.ib_user, &query.tag, session_uid).await
  } else {
    render_search_posts_html(&state, query.ib_uid, &query.ib_user, &query.tag, session_uid).await
  };
  match html_result {
    Ok(html) => HttpResponse::Ok()
      .content_type("text/html; charset=utf-8")
      .body(html),
    Err(err) => HttpResponse::InternalServerError().body(err),
  }
}
#[get("/v1/searchprojects")]
pub async fn search_projects(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<SearchProjectsRequest>,
) -> impl Responder {
  let session_uid = get_session_uid(&req);
  let html_result = if is_mobile_device(&req) {
    render_search_projects_mobile_html(&state, query.ib_uid, &query.ib_user, &query.query, session_uid).await
  } else {
    render_search_projects_html(&state, query.ib_uid, &query.ib_user, &query.query, session_uid).await
  };
  match html_result {
    Ok(html) => HttpResponse::Ok()
      .content_type("text/html; charset=utf-8")
      .body(html),
    Err(err) => HttpResponse::InternalServerError().body(err),
  }
}
#[get("/v1/search-section")]
pub async fn search_section(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<SearchSectionRequest>,
) -> impl Responder {
  let session_uid = get_session_uid(&req);
  match render_user_search_section_html(&state, query.ib_uid, &query.ib_user, session_uid).await {
    Ok(html) => HttpResponse::Ok()
      .content_type("text/html; charset=utf-8")
      .body(html),
    Err(err) => HttpResponse::InternalServerError().body(err),
  }
}
#[get("/v1/projects")]
pub async fn projects_page(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<ProjectsRequest>,
) -> impl Responder {
  let session_uid = get_session_uid(&req);
  let html_result = if is_mobile_device(&req) {
    render_projects_mobile_html(&state, query.ib_uid, &query.ib_user, session_uid).await
  } else {
    render_projects_html(&state, query.ib_uid, &query.ib_user, session_uid).await
  };

  match html_result {
    Ok(html) => HttpResponse::Ok()
      .content_type("text/html; charset=utf-8")
      .body(html),
    Err(err) => HttpResponse::InternalServerError().body(err),
  }
}
#[get("/v1/admin/ads")]
pub async fn ads_admin_page(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<AdsAdminRequest>,
) -> impl Responder {
  if !is_expected_ad_admin_identity(query.ib_uid, &query.ib_user)
    || !is_ad_admin_session(&req, &state).await
  {
    return HttpResponse::Forbidden().body("Forbidden");
  }

  let rows = match sqlx::query_as::<_, AdvertImageAdminRow>(
    "SELECT imageid, imagepath, url, COALESCE(clicks, 0) AS clicks, COALESCE(views, 0) AS views FROM advert_image ORDER BY imageid DESC",
  )
  .fetch_all(&state.db_pool)
  .await
  {
    Ok(rows) => rows,
    Err(err) => {
      return HttpResponse::InternalServerError().body(format!("Ad list query failed: {}", err));
    }
  };

  let mut ad_rows_html = String::new();

  for row in rows {
    ad_rows_html += &format!(
      r#"<div class="post" style="margin-bottom:16px;">
  <p><strong>ID:</strong> {imageid}</p>
  <p><strong>Views:</strong> {views} | <strong>Clicks:</strong> {clicks}</p>
  <p><strong>Preview:</strong><br><img src="{imagepath}" width="400" height="111" alt="{imageid}"></p>
  <form id="ad-update-{imageid}" action="https://{DOMAIN}/v1/admin/ads/update" method="POST">
    <input type="hidden" name="ib_uid" value="{ib_uid}">
    <input type="hidden" name="ib_user" value="{ib_user}">
    <input type="hidden" name="imageid" value="{imageid}">
    <p>Image Path: <input class="post" type="text" name="imagepath" value="{imagepath}" maxlength="1024" required></p>
    <p>Target URL: <input class="post" type="text" name="url" value="{url}" maxlength="2048" required></p>
  </form>
  <div style="display:flex; justify-content:center; align-items:center; gap:10px; margin-top:8px;">
    <input class="post-submit" type="submit" form="ad-update-{imageid}" value="Update Ad" style="position:static; left:0; margin:0;">
    <form action="https://{DOMAIN}/v1/admin/ads/delete" method="POST" style="display:inline-flex; margin:0;">
      <input type="hidden" name="ib_uid" value="{ib_uid}">
      <input type="hidden" name="ib_user" value="{ib_user}">
      <input type="hidden" name="imageid" value="{imageid}">
      <input class="post-cancel" type="submit" value="Delete Ad" style="position:static; left:0; margin:0;">
    </form>
  </div>
</div>"#,
      ib_uid = AD_ADMIN_UID,
      ib_user = AD_ADMIN_USER,
      imageid = row.imageid,
      imagepath = escape_html(&row.imagepath),
      url = escape_html(&row.url),
      clicks = row.clicks,
      views = row.views,
    );
  }

  let html = format!(
    r#"<!DOCTYPE html>
<html lang="en-US">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <link rel="stylesheet" type="text/css" href="/css/is-by.css" />
  <title>Ad Admin</title>
</head>
<body>
  <div id="main-section">
    <div id="media-section">
      <div id="navigation-section">
        <a class="pro-home-display" href="https://{DOMAIN}/v1/profile/{ib_user}">:[[ :profile-home: ]]:</a>
      </div>
      <div id="selected-user-posts-section" class="post-section">
        <div class="notice"><p><em>Ad Admin Panel</em></p></div>
        <div class="post" style="margin-bottom:16px;">
          <form action="https://{DOMAIN}/v1/admin/ads/create" method="POST">
            <input type="hidden" name="ib_uid" value="{ib_uid}">
            <input type="hidden" name="ib_user" value="{ib_user}">
            <p>Image Path: <input class="post" type="text" name="imagepath" maxlength="1024" placeholder="/images/advert/example.png" required></p>
            <p>Target URL: <input class="post" type="text" name="url" maxlength="2048" placeholder="https://example.com" required></p>
            <input class="post-submit" type="submit" value="Create Ad">
          </form>
        </div>
        {ad_rows_html}
      </div>
    </div>
  </div>
</body>
</html>"#,
    ib_uid = AD_ADMIN_UID,
    ib_user = AD_ADMIN_USER,
    ad_rows_html = ad_rows_html,
  );

  HttpResponse::Ok()
    .content_type("text/html; charset=utf-8")
    .body(html)
}
#[get("/v1/ads")]
pub async fn ads_user_page(
  req: HttpRequest,
  state: web::Data<AppState>,
) -> impl Responder {
  let Some((session_uid, session_user)) = get_session_identity(&req, &state).await else {
    return HttpResponse::Unauthorized().body("Login required");
  };

  let rows = match sqlx::query_as::<_, AdvertOwnedRow>(
    "SELECT imageid, imagepath, url, COALESCE(clicks, 0) AS clicks, COALESCE(views, 0) AS views, COALESCE(payment_status, 'pending') AS payment_status FROM advert_image WHERE owner_uid = ? ORDER BY imageid DESC",
  )
  .bind(session_uid)
  .fetch_all(&state.db_pool)
  .await
  {
    Ok(rows) => rows,
    Err(err) => {
      return HttpResponse::InternalServerError().body(format!("Ad query failed: {}", err));
    }
  };

  let mut ad_rows_html = String::new();
  for row in rows {
    let pay_now_html = if row.payment_status == "paid" {
      String::new()
    } else {
      format!(
        r#"<form action="https://{DOMAIN}/v1/ads/pay/{imageid}" method="POST" style="display:inline-flex; margin:0 0 0 10px;"><input class="post-submit" type="submit" value="Pay with PayPal" style="position:static; left:0; margin:0;"></form>"#,
        imageid = row.imageid,
      )
    };

    ad_rows_html += &format!(
      r#"<div class="post" style="margin-bottom:16px;">
  <p><strong>ID:</strong> {imageid} | <strong>Status:</strong> {status}</p>
  <p><strong>Views:</strong> {views} | <strong>Clicks:</strong> {clicks}</p>
  <p><img src="{imagepath}" width="400" height="111" alt="{imageid}"></p>
  <form id="ad-user-update-{imageid}" action="https://{DOMAIN}/v1/ads/update" method="POST">
    <input type="hidden" name="imageid" value="{imageid}">
    <p>Target URL: <input class="post" type="text" name="url" value="{url}" maxlength="2048" required></p>
  </form>
  <div style="display:flex; justify-content:center; align-items:center; gap:10px; margin-top:8px;">
    <input class="post-submit" type="submit" form="ad-user-update-{imageid}" value="Update Ad" style="position:static; left:0; margin:0;">
    <form action="https://{DOMAIN}/v1/ads/delete" method="POST" style="display:inline-flex; margin:0;">
      <input type="hidden" name="imageid" value="{imageid}">
      <input class="post-cancel" type="submit" value="Delete Ad" style="position:static; left:0; margin:0;">
    </form>
    {pay_now_html}
  </div>
</div>"#,
      imageid = row.imageid,
      status = escape_html(&row.payment_status),
      views = row.views,
      clicks = row.clicks,
      imagepath = escape_html(&row.imagepath),
      url = escape_html(&row.url),
      pay_now_html = pay_now_html,
    );
  }

  let html = format!(
    r#"<!DOCTYPE html>
<html lang="en-US">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <link rel="stylesheet" type="text/css" href="/css/is-by.css" />
  <title>My Ads</title>
</head>
<body>
  <div id="main-section">
    <div id="media-section">
      <div id="navigation-section">
        <a class="pro-home-display" href="https://{DOMAIN}/v1/profile/{ib_user}">:[[ :profile-home: ]]:</a>
      </div>
      <div id="selected-user-posts-section" class="post-section">
        <div class="notice"><p><em>My Ads (PayPal + 400x111 upload)</em></p></div>
        <div class="post" style="margin-bottom:16px;">
          <form action="https://{DOMAIN}/v1/ads/create" method="POST" enctype="multipart/form-data">
            <p>Target URL: <input class="post" type="text" name="url" maxlength="2048" placeholder="https://example.com" required></p>
            <p>Image (must be exactly 400x111): <input type="file" name="ad_image" accept="image/png,image/jpeg,image/gif,image/webp" required></p>
            <input class="post-submit" type="submit" value="Upload + Pay with PayPal" style="position:static; left:0; display:block; width:auto; min-width:280px; padding:6px 16px; margin:12px auto 0 auto;">
          </form>
        </div>
        {ad_rows_html}
      </div>
    </div>
  </div>
</body>
</html>"#,
    ib_user = escape_html(&session_user),
    ad_rows_html = ad_rows_html,
  );

  HttpResponse::Ok()
    .content_type("text/html; charset=utf-8")
    .body(html)
}
#[get("/v1/warroom")]
pub async fn war_room(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<WarRoomRequest>,
) -> impl Responder {
  let session_uid = get_session_uid(&req);
  if session_uid.is_none() {
    return HttpResponse::Unauthorized().body("Login required");
  }

  if is_mobile_device(&req) {
    match render_war_room_mobile_html(&state, query.ib_uid, &query.ib_user, session_uid).await {
      Ok(html) => HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html),
      Err(err) => HttpResponse::InternalServerError().body(err),
    }
  } else {
    match render_war_room_html(&state, query.ib_uid, &query.ib_user, session_uid).await {
      Ok(html) => HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html),
      Err(err) => HttpResponse::InternalServerError().body(err),
    }
  }
}
#[get("/v1/inbox")]
pub async fn inbox(
  req: HttpRequest,
  state: web::Data<AppState>,
  query: web::Query<InboxRequest>,
) -> impl Responder {
  let session_uid = get_session_uid(&req);
  if session_uid.is_none() {
    return HttpResponse::Unauthorized().body("Login required");
  }

  let html_result = if is_mobile_device(&req) {
    render_inbox_mobile_html(&state, query.ib_uid, &query.ib_user, session_uid, query.target_user.as_deref()).await
  } else {
    render_inbox_html(&state, query.ib_uid, &query.ib_user, session_uid, query.target_user.as_deref()).await
  };

  match html_result {
    Ok(html) => HttpResponse::Ok()
      .content_type("text/html; charset=utf-8")
      .body(html),
    Err(err) => HttpResponse::InternalServerError().body(err),
  }
}
#[get("/")]
pub async fn hello(req: HttpRequest) -> impl Responder {
  if is_mobile_device(&req) {
     return HttpResponse::SeeOther().insert_header(("Location", "/mobile.html")).finish();
  }

  let mut context = tera::Context::new();
  context.insert("copyright", &COPYRIGHT);
  let html = TEMPLATES.render("index.html", &context)
        .unwrap_or_else(|e| {
            use std::error::Error;
            let mut err_msg = format!("Template error: {}", e);
            let mut cause = e.source();
            while let Some(err) = cause {
                err_msg.push_str(&format!("\nCaused by: {}", err));
                cause = err.source();
            }
            err_msg
        });
  HttpResponse::Ok().body(html)
}
#[get("/mobile.html")]
pub async fn mobile_shell_html() -> impl Responder {
  HttpResponse::Ok()
    .insert_header(("Cache-Control", "no-cache, no-store, must-revalidate"))
    .insert_header(("Pragma", "no-cache"))
    .insert_header(("Expires", "0"))
    .content_type("text/html; charset=utf-8")
    .body(include_str!("../../webroot/mobile.html"))
}
#[get("/app.webmanifest")]
pub async fn mobile_shell_manifest() -> impl Responder {
  HttpResponse::Ok()
    .insert_header(("Cache-Control", "no-cache, no-store, must-revalidate"))
    .insert_header(("Pragma", "no-cache"))
    .insert_header(("Expires", "0"))
    .content_type("application/manifest+json")
    .body(include_str!("../../webroot/app.webmanifest"))
}
#[get("/sw.js")]
pub async fn mobile_shell_service_worker() -> impl Responder {
  HttpResponse::Ok()
    .insert_header(("Cache-Control", "no-cache, no-store, must-revalidate"))
    .insert_header(("Pragma", "no-cache"))
    .insert_header(("Expires", "0"))
    .content_type("text/javascript; charset=utf-8")
    .body(include_str!("../../webroot/sw.js"))
}

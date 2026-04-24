import sys

with open("src/main.rs", "r") as f:
    content = f.read()

start_sig = "async fn render_search_posts_html("
end_sig = "async fn render_projects_html("

start_idx = content.find(start_sig)
end_idx = content.find(end_sig)

if start_idx == -1 or end_idx == -1:
    print("Could not find function signatures.")
    sys.exit(1)

new_func = """async fn render_search_posts_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  raw_tag: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

  let normalized_tag = normalize_hashtag(raw_tag);
  let tag = normalized_tag.as_deref().unwrap_or(raw_tag.trim_start_matches('#')).to_string();

  let mut post_html = String::new();

  if let Some(ref valid_tag) = normalized_tag {
    let pattern = format!(
      r"(^|[^[:alnum:]_])#{}([^[:alnum:]_]|$)",
      escape_mysql_regex_token(valid_tag)
    );

    let rows = sqlx::query_as::<_, PostRow>(
      "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE LOWER(COALESCE(post.post, '')) REGEXP ? ORDER BY post.timestamp DESC LIMIT 200"
      )
      .bind(&pattern)
      .fetch_all(&state.db_pool)
      .await
      .map_err(|e| format!("Search posts query failed: {}", e))?;

    if rows.is_empty() {
      post_html = "<p><em>:[[ :search-posts: is-by: no: is-with: results: ]]:</em></p>".to_string();
    } else {
      let row_post_ids: Vec<String> = rows.iter().map(|row| row.postid.clone()).collect();
      let acknowledged_post_ids = acknowledged_post_ids_for_user(&state.db_pool, session_uid, &row_post_ids).await;

      for row in rows {
        post_html += &format!(
          r#"<div class="post" data-postid="{post_id}" data-timestamp="{post_timestamp}">
            {post_meta}
            <p>{post_body}</p>
            <div class="post-actions">
              {ack_controls}
              <form class="show-post-form" action="https://{DOMAIN}/v1/showpost" method="GET">
                <input type="hidden" name="ib_uid" value="{post_owner_uid}">
                <input type="hidden" name="ib_user" value="{post_owner_user}">
                <input type="hidden" name="pid" value="{post_id}">
              </form>
              <a href="javascript:void(0);" class="show-post">:[[ :show-post: ]]:</a>
            </div>
            <p class="acknowledged-count">Acknowleged {acknowledged_count} times.</p>
          </div>"#,
          post_id = escape_html(&row.postid),
          post_timestamp = escape_html(&row.timestamp),
          post_meta = render_post_meta(&row.ib_uid, &row.username, &row.timestamp, row.user_total_acks),
          post_body = render_post_with_hashtags(&row.post, ib_uid, ib_user),
          ack_controls = if session_uid.is_none() || acknowledged_post_ids.contains(&row.postid) {
            render_ack_disabled()
          } else {
            render_ack_controls(ib_uid, ib_user, &row.postid)
          },
          acknowledged_count = row.acknowledged_count,
          post_owner_uid = escape_html(&row.ib_uid),
          post_owner_user = escape_html(&row.username)
        );
      }
    }
  } else {
    post_html = "<p><em>:[[ :search-posts: invalid-tag: ]]:</em></p>".to_string();
  }

  let session_username = if let Some(uid) = session_uid {
    match sqlx::query_as::<_, SessionUserRow>(
      "SELECT username FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1",
    )
    .bind(uid.to_string())
    .fetch_optional(&state.db_pool)
    .await
    {
      Ok(Some(row)) if !row.username.trim().is_empty() => Some(row.username),
      _ => None,
    }
  } else {
    None
  };

  let unread_dm_count = if let Some(uid) = session_uid {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM dm WHERE recipient_uid = ? AND read_at IS NULL")
      .bind(uid)
      .fetch_one(&state.db_pool)
      .await
      .unwrap_or(0)
  } else {
    0
  };

  let navigation_links = if session_uid.is_some() {
    let session_nav_uid = session_uid.unwrap_or(ib_uid);
    let session_nav_user = session_username.as_deref().unwrap_or(ib_user);

    format!(
      r#"<a class="post-form-display" href="javascript:void(0);">:[[ :post: ]]:</a>
        <a class="pro-home-display" href="https://{DOMAIN}/v1/profile/{session_ib_user}">:[[ :profile-home: ]]:</a>
        <a class="war-room-display" href="https://{DOMAIN}/v1/warroom?ib_uid={session_ib_uid}&ib_user={session_ib_user}">:[[ :war-room: ]]:</a>
        <a class="projects-display" href="https://{DOMAIN}/v1/projects?ib_uid={session_ib_uid}&ib_user={session_ib_user}">:[[ :projects: ]]:</a>
        <a class="dm-inbox-display" href="https://{DOMAIN}/v1/inbox?ib_uid={session_ib_uid}&ib_user={session_ib_user}"><svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: middle; margin-right: 4px;"><path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path><polyline points="22,6 12,13 2,6"></polyline></svg> <span id="dm-unread-count">{unread_dm_count}</span></a>"#,
      session_ib_uid = session_nav_uid,
      session_ib_user = escape_html(session_nav_user),
      unread_dm_count = unread_dm_count
    )
  } else {
    String::new()
  };

  let mut ib_ibp = String::new();
  let mut ib_pro_str = String::new();
  let mut ib_services = String::new();
  let mut ib_location = String::new();
  let mut ib_website = String::new();
  let mut edit_profile_link = String::new();
  let mut related_userlist_html = String::new();
  let mut trending_tags_html = String::new();
  let mut github_identity_html = String::new();
  let mut sidebar_login_html = String::new();

  let ib_pro_result = sqlx::query_as::<_, ProRow>(
      "SELECT ibp, pro, location, services, website, github FROM pro WHERE ib_uid = ?"
    )
    .bind(ib_uid)
    .fetch_one(&state.db_pool)
    .await;

  if let Ok(ib_pro) = ib_pro_result {
    ib_ibp = escape_html(&ib_pro.ibp);
    ib_pro_str = escape_html(&ib_pro.pro);
    ib_services = escape_html(&ib_pro.services);
    ib_location = escape_html(&ib_pro.location);
    ib_website = escape_html(&ib_pro.website);

    let source_uid = session_uid.unwrap_or(ib_uid);
    let source_profile_terms = if let Some(uid) = session_uid {
      lookup_profile_terms_by_uid(state, uid)
        .await
        .unwrap_or_else(|| format!("{} {}", ib_pro.pro, ib_pro.ibp))
    } else {
      format!("{} {}", ib_pro.pro, ib_pro.ibp)
    };
    
    let show_edit_profile_link = session_username
      .as_ref()
      .map(|u| u.eq_ignore_ascii_case(ib_user))
      .unwrap_or(false);

    if show_edit_profile_link {
      edit_profile_link = format!(r#"<p><a href="https://{DOMAIN}/v1/editprofile?ib_uid={ib_uid}&ib_user={ib_user}">:[[ :edit-profile: ]]:</a></p><br>"#, ib_uid = ib_uid, ib_user = escape_html(ib_user));
    }
  
    related_userlist_html = render_related_userlist_html(state, session_uid, source_uid, &source_profile_terms).await;
    trending_tags_html = render_trending_tags_html(state, ib_uid, ib_user).await;
    github_identity_html = render_github_identity_html(state, ib_user).await;
    if session_uid.is_none() {
      sidebar_login_html = r#"<div id="actions-section">
      <div class="login-section">
        <p><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#.to_string();
    }
  } else {
    if session_uid.is_none() {
      sidebar_login_html = r#"<div id="actions-section">
      <div class="login-section">
        <p><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#.to_string();
    }
  }

  let follower_list = match sqlx::query_scalar::<_, String>("SELECT followers FROM user WHERE ib_uid = ?")
    .bind(ib_uid)
    .fetch_optional(&state.db_pool)
    .await
  {
    Ok(Some(followers)) => parse_comma_separated_string(&followers),
    _ => Vec::new(),
  };

  let show_unfollow = session_username
    .as_ref()
    .map(|username| {
      follower_list
        .iter()
        .any(|follower| follower.eq_ignore_ascii_case(username))
    })
    .unwrap_or(false);

  let show_follow = session_username
    .as_ref()
    .map(|username| !show_unfollow && !username.eq_ignore_ascii_case(ib_user))
    .unwrap_or(false);

  let follow_form_html = if show_follow {
    format!(
      r#"<form id="follow-form" action="https://{DOMAIN}/v1/follow" method="POST">
        <input type="hidden" name="target_user" value="{target_user}">
        <input type="submit" value="Follow">
      </form>"#,
      target_user = escape_html(ib_user)
    )
  } else {
    String::new()
  };

  let unfollow_form_html = if show_unfollow {
    format!(
      r#"<form id="unfollow-form" action="https://{DOMAIN}/v1/unfollow" method="POST">
        <input type="hidden" name="target_user" value="{target_user}">
        <input type="submit" value="Unfollow">
      </form>"#,
      target_user = escape_html(ib_user)
    )
  } else {
    String::new()
  };
  
  let follow_section_html = format!(
      r#"<div id="follow-section">
      {follow_form_html}
      {unfollow_form_html}
    </div>"#,
    follow_form_html = follow_form_html,
    unfollow_form_html = unfollow_form_html
  );

  let total_acks = match sqlx::query_as::<_, UserHoverLookupRow>(
    "SELECT CONVERT(ib_uid USING utf8mb4) AS ib_uid, username, COALESCE(followers, '') AS followers, COALESCE(total_acknowledgments, 0) AS total_acknowledgments FROM user WHERE LOWER(username) = LOWER(?) LIMIT 1",
  )
  .bind(ib_user)
  .fetch_optional(&state.db_pool)
  .await
  {
    Ok(Some(row)) => row.total_acknowledgments,
    _ => 0,
  };
  let (rank_level, rank_name) = rank_from_unique_acknowledgments(total_acks);

  let post_form_html = if session_uid.is_some() {
    let session_nav_uid = session_uid.unwrap_or(ib_uid);
    let session_nav_user = session_username.as_deref().unwrap_or(ib_user);
    format!(
      r#"<div id="post-form-section">
          <form id="post-form" action="https://{DOMAIN}/v1/post" method="POST">
            <div id="post-message"></div>
            <div id="post-character-count"></div>
            <input type="hidden" name="ib_uid" value="{session_uid}">
            <input type="hidden" name="ib_user" value="{session_user}">
            <input class="post" type="text" name="post" autocomplete="off" maxlength="1024" required>
            <input id="post-cancel" class="post-cancel" type="button" value="Cancel">
            <input class="post-submit" type="submit" value="Post">
          </form>
        </div>"#,
      session_uid = session_nav_uid,
      session_user = escape_html(session_nav_user)
    )
  } else {
    String::new()
  };

  context.insert("post_html", &post_html);
  context.insert("tag", &tag);
  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &ib_user);
  context.insert("advert_html", &advert_html);
  context.insert("navigation_links", &navigation_links);
  context.insert("post_form_html", &post_form_html);
  context.insert("edit_profile_link", &edit_profile_link);
  context.insert("github_identity_html", &github_identity_html);
  context.insert("sidebar_login_html", &sidebar_login_html);
  context.insert("related_userlist_html", &related_userlist_html);
  context.insert("trending_tags_html", &trending_tags_html);
  context.insert("domain", &DOMAIN);
  context.insert("viewed_ib_uid", &ib_uid);
  context.insert("viewed_ib_user", &ib_user);
  context.insert("rank_name", &rank_name);
  context.insert("rank_level", &rank_level);
  context.insert("ib_ibp", &ib_ibp);
  context.insert("ib_pro", &ib_pro_str);
  context.insert("ib_services", &ib_services);
  context.insert("ib_location", &ib_location);
  context.insert("ib_website", &ib_website);
  context.insert("follow_section_html", &follow_section_html);
  
  let html = TEMPLATES.render("search_posts.html", &context)
    .map_err(|e| {
        use std::error::Error;
        let mut err_msg = format!("Template error: {}", e);
        let mut cause = e.source();
        while let Some(err) = cause {
            err_msg.push_str(&format!("\\nCaused by: {}", err));
            cause = err.source();
        }
        err_msg
    })?;

  Ok(html)
}
"""

content = content[:start_idx] + new_func + content[end_idx:]

with open("src/main.rs", "w") as f:
    f.write(content)


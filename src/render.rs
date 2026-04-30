use crate::{models::*, db::*, utils::*};
use is_by_pro::COPYRIGHT;
use actix_web::{HttpRequest, HttpResponse};
use std::collections::HashSet;
use tera::Context;
use crate::DOMAIN;

pub async fn render_related_userlist_html(
  state: &AppState,
  session_uid: Option<i64>,
  source_uid: i64,
  source_profile_text: &str,
) -> String {
  let cache_key = format!("cache:related_users:{}:{:?}", source_uid, session_uid);
  if let Some(cached_html) = crate::utils::get_cache(&state.redis_pool, &cache_key).await {
    return cached_html;
  }

  let followed_usernames = lookup_following_usernames(state, session_uid).await;
  let follower_usernames = lookup_follower_usernames(state, session_uid).await;
  let mut excluded_usernames = followed_usernames;
  excluded_usernames.extend(follower_usernames);

  let mut seen = HashSet::new();
  let raw_terms: Vec<String> = source_profile_text
    .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '+' || ch == '#'))
    .map(|term| term.trim().to_lowercase())
    .filter(|term| !term.is_empty())
    .filter(|term| seen.insert(term.clone()))
    .collect();

  let interests: Vec<String> = {
    let filtered: Vec<String> = raw_terms
      .iter()
      .filter(|term| term.len() >= 3)
      .cloned()
      .collect();
    if filtered.is_empty() {
      raw_terms.clone()
    } else {
      filtered
    }
  };

  if interests.is_empty() {
    return "<p><em>:[[ :is-by: none: for-the: related-users: ]]:</em></p>".to_string();
  }

  let regex_terms: Vec<String> = interests
    .iter()
    .map(|term| escape_mysql_regex_token(term))
    .collect();
  let pattern = format!("({})", regex_terms.join("|"));
  let source_uid_text = source_uid.to_string();

  let related_rows = sqlx::query_as::<_, RelatedUsernameRankRow>(
      "SELECT CAST(COALESCE(CONVERT(user.username USING utf8mb4), '') AS CHAR CHARACTER SET utf8mb4) AS username, COALESCE(user.total_acknowledgments, 0) AS total_acknowledgments FROM user AS user LEFT JOIN pro AS candidate ON CONVERT(candidate.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci WHERE CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci <> ? COLLATE utf8mb4_unicode_ci AND LOWER(COALESCE(CONVERT(user.username USING utf8mb4), '')) <> '' AND (LOWER(COALESCE(CONVERT(user.username USING utf8mb4), '')) REGEXP ? OR LOWER(COALESCE(candidate.github, '')) REGEXP ? OR LOWER(COALESCE(candidate.ibp, '')) REGEXP ? OR LOWER(COALESCE(candidate.pro, '')) REGEXP ? OR LOWER(COALESCE(candidate.services, '')) REGEXP ? OR LOWER(COALESCE(candidate.location, '')) REGEXP ? OR LOWER(COALESCE(candidate.website, '')) REGEXP ?) ORDER BY RAND() LIMIT 5"
    )
    .bind(&source_uid_text)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .bind(&pattern)
    .fetch_all(&state.db_pool)
    .await;

  let mut related_rows: Vec<RelatedUsernameRankRow> = match related_rows {
    Ok(rows) => rows
      .into_iter()
      .filter(|row| !excluded_usernames.contains(&row.username.to_lowercase()))
      .collect(),
    Err(e) => return format!("<p><em>:[[ :related-user-lookup: failed: {} ]]:</em></p>", e),
  };

  if related_rows.is_empty() {
    let mut sql = String::from(
      "SELECT CAST(COALESCE(CONVERT(user.username USING utf8mb4), '') AS CHAR CHARACTER SET utf8mb4) AS username, COALESCE(user.total_acknowledgments, 0) AS total_acknowledgments FROM user AS user LEFT JOIN pro AS candidate ON CONVERT(candidate.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci WHERE CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci <> ? COLLATE utf8mb4_unicode_ci AND LOWER(COALESCE(CONVERT(user.username USING utf8mb4), '')) <> '' AND ("
    );

    for index in 0..interests.len() {
      if index > 0 {
        sql.push_str(" OR ");
      }
      sql.push_str("LOWER(COALESCE(CONVERT(user.username USING utf8mb4), \"\")) LIKE ? OR LOWER(COALESCE(candidate.github, \"\")) LIKE ? OR LOWER(COALESCE(candidate.ibp, \"\")) LIKE ? OR LOWER(COALESCE(candidate.pro, \"\")) LIKE ? OR LOWER(COALESCE(candidate.services, \"\")) LIKE ? OR LOWER(COALESCE(candidate.location, \"\")) LIKE ? OR LOWER(COALESCE(candidate.website, \"\")) LIKE ?");
    }

    sql.push_str(") ORDER BY RAND() LIMIT 5");

    let mut query = sqlx::query_as::<_, RelatedUsernameRankRow>(&sql).bind(&source_uid_text);
    for term in &interests {
      let token = format!("%{}%", escape_mysql_like_token(term));
      query = query
        .bind(token.clone())
        .bind(token.clone())
        .bind(token.clone())
        .bind(token.clone())
        .bind(token.clone())
        .bind(token.clone())
        .bind(token);
    }

    related_rows = match query.fetch_all(&state.db_pool).await {
      Ok(rows) => rows
        .into_iter()
        .filter(|row| !excluded_usernames.contains(&row.username.to_lowercase()))
        .collect(),
      Err(e) => return format!("<p><em>:[[ :related-user-lookup: failed: {} ]]:</em></p>", e),
    };
  }

  if related_rows.is_empty() {
    let candidates = match sqlx::query_as::<_, RelatedCandidateRow>(
      "SELECT CAST(COALESCE(CONVERT(user.username USING utf8mb4), '') AS CHAR CHARACTER SET utf8mb4) AS username, COALESCE(user.total_acknowledgments, 0) AS total_acknowledgments, COALESCE(candidate.github, '') AS github, COALESCE(candidate.ibp, '') AS ibp, COALESCE(candidate.pro, '') AS pro, COALESCE(candidate.services, '') AS services, COALESCE(candidate.location, '') AS location, COALESCE(candidate.website, '') AS website FROM user AS user LEFT JOIN pro AS candidate ON CONVERT(candidate.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci WHERE CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci <> ? COLLATE utf8mb4_unicode_ci ORDER BY RAND() LIMIT 250"
    )
    .bind(&source_uid_text)
    .fetch_all(&state.db_pool)
    .await
    {
      Ok(rows) => rows,
      Err(_) => return "<p><em>:[[ :related-user-lookup: failed: ]]:</em></p>".to_string(),
    };

    for candidate in candidates {
      if candidate.username.trim().is_empty() {
        continue;
      }

      let haystack = format!(
        "{} {} {} {} {} {} {}",
        candidate.username,
        candidate.github,
        candidate.ibp,
        candidate.pro,
        candidate.services,
        candidate.location,
        candidate.website
      )
      .to_lowercase();

      if interests.iter().any(|term| haystack.contains(term)) {
        if excluded_usernames.contains(&candidate.username.to_lowercase()) {
          continue;
        }

        related_rows.push(RelatedUsernameRankRow {
          username: candidate.username,
          total_acknowledgments: candidate.total_acknowledgments,
        });

        if related_rows.len() >= 5 {
          break;
        }
      }
    }
  }

  if related_rows.is_empty() {
    related_rows = match sqlx::query_as::<_, RelatedUsernameRankRow>(
      "SELECT CAST(COALESCE(CONVERT(username USING utf8mb4), '') AS CHAR CHARACTER SET utf8mb4) AS username, COALESCE(total_acknowledgments, 0) AS total_acknowledgments FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci <> ? COLLATE utf8mb4_unicode_ci AND LOWER(COALESCE(CONVERT(username USING utf8mb4), '')) <> '' ORDER BY RAND() LIMIT 5"
    )
    .bind(&source_uid_text)
    .fetch_all(&state.db_pool)
    .await
    {
      Ok(rows) => rows
        .into_iter()
        .filter(|row| !excluded_usernames.contains(&row.username.to_lowercase()))
        .collect(),
      Err(_) => return "<p><em>:[[ :related-user-lookup: failed: ]]:</em></p>".to_string(),
    };
  }

  let mut related_html = String::new();

  for username_row in related_rows {
    if username_row.username.trim().is_empty() {
      continue;
    }

    related_html += &format!(
      "<br><p>{}</p>",
      render_project_profile_link(&username_row.username, username_row.total_acknowledgments)
    );
  }

  let html = if related_html.is_empty() {
    "<p><em>:[[ :is-by: none: for-the: related-users: ]]:</em></p>".to_string()
  } else {
    related_html
  };

  crate::utils::set_cache(&state.redis_pool, &cache_key, &html, 300).await;
  html
}

pub async fn render_github_identity_html(state: &AppState, ib_user: &str) -> String {
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

  let rank_info = get_rank_info(total_acks);
  let glow_style = if rank_info.level >= 11 {
    "filter: drop-shadow(0 0 3px #fff) drop-shadow(0 0 5px #fff); "
  } else {
    ""
  };
  let safe_user = escape_html(ib_user);
  let encoded_user = url_encode_component(ib_user);

  format!(
    r#"<a class="post-author" target="_blank" rel="noopener" href="https://github.com/{encoded_user}"><img class="post-author-avatar" src="https://github.com/{encoded_user}.png?size=32" alt="{safe_user}" width="32" height="32" style="margin-right:6px;vertical-align:middle;"><img class="rank-insignia" src="/images/ranks/{rank_icon}" alt="Rank" width="16" height="16" style="{glow_style}vertical-align: middle; margin-left: 4px; margin-right: 4px;">{safe_user}</a>"#,
    encoded_user = encoded_user,
    safe_user = safe_user,
    rank_icon = rank_info.asset,
    glow_style = glow_style,
  )
}

pub async fn render_advert_html(state: &AppState) -> String {
  const FALLBACK_IMAGE: &str = "/images/advert/Death_Angel-Ad-400x111.png";
  const FALLBACK_URL: &str = "https://is-by.pro/advertise.html";

  let ad_row = sqlx::query_as::<_, AdvertImageRow>(
      "SELECT imageid, imagepath, url FROM advert_image WHERE payment_status = 'paid' ORDER BY RAND() LIMIT 1"
    )
    .fetch_optional(&state.db_pool)
    .await;

  let Some(ad_row) = ad_row.ok().flatten() else {
    return format!(
      r#"<div class="sponsor-content">
        <a href="{url}" target="_blank" rel="noopener noreferrer"><img src="{imagepath}" width="400" height="111" alt="fallback"></a>
      </div>"#,
      url = FALLBACK_URL,
      imagepath = FALLBACK_IMAGE,
    );
  };

  let _ = sqlx::query(
    "UPDATE advert_image SET views = COALESCE(views, 0) + 1 WHERE imageid = ? LIMIT 1",
  )
  .bind(ad_row.imageid)
  .execute(&state.db_pool)
  .await;

  format!(
    r#"<a href="https://{DOMAIN}/v1/ad/click/{imageid}"><img src="{imagepath}" width="400" height="111" alt="{imageid}"></a>"#,
    imageid = ad_row.imageid,
    imagepath = escape_html(&ad_row.imagepath),
  )
}

pub async fn render_trending_tags_html(state: &AppState, ib_uid: i64, ib_user: &str) -> String {
  let cache_key = format!("cache:trending_tags:{}:{}", ib_uid, ib_user);
  if let Some(cached_html) = crate::utils::get_cache(&state.redis_pool, &cache_key).await {
    return cached_html;
  }

  let rows = sqlx::query_as::<_, TrendingTagRow>(
      "SELECT tag, COUNT(*) AS tag_count FROM post_tag WHERE created_at >= (NOW() - INTERVAL 1 DAY) GROUP BY tag ORDER BY tag_count DESC, tag ASC LIMIT 25"
    )
    .fetch_all(&state.db_pool)
    .await;

  let html = match rows {
    Ok(rows) if rows.is_empty() => "<p><em>:[[ :is-by: none: for-the: trending-tags: ]]:</em></p>".to_string(),
    Ok(rows) => rows
      .iter()
      .map(|row| {
        let href = format!(
          "https://{DOMAIN}/v1/searchposts?ib_uid={ib_uid}&ib_user={ib_user}&tag=%23{tag}",
          ib_uid = ib_uid,
          ib_user = url_encode_component(ib_user),
          tag = url_encode_component(&row.tag)
        );

        format!(
          r#"<p><a href="{href}">#{tag}</a> ({count})</p>"#,
          href = href,
          tag = escape_html(&row.tag),
          count = row.tag_count
        )
      })
      .collect::<Vec<String>>()
      .join(""),
    Err(_) => "<p><em>:[[ :trending-tags: unavailable: ]]:</em></p>".to_string(),
  };

  crate::utils::set_cache(&state.redis_pool, &cache_key, &html, 300).await;
  html
}

pub async fn render_profile_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

  let viewed_user_row = sqlx::query_as::<_, FollowLookupRow>(
      "SELECT username, COALESCE(followers, '') AS followers FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1"
    )
    .bind(ib_uid.to_string())
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| format!("Viewed user lookup failed: {}", e))?;

  let viewed_username = viewed_user_row
    .as_ref()
    .map(|row| row.username.clone())
    .unwrap_or_else(|| ib_user.to_string());

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

  let follower_list: Vec<String> = viewed_user_row
    .as_ref()
    .map(|row| {
      row
        .followers
        .split(',')
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
    })
    .unwrap_or_default();

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
    .map(|username| !show_unfollow && !username.eq_ignore_ascii_case(&viewed_username))
    .unwrap_or(false);

  let follow_form_html = if show_follow {
    format!(
      r#"<form id="follow-form" action="https://{DOMAIN}/v1/follow" method="POST">
        <input type="hidden" name="target_user" value="{target_user}">
        <input type="submit" value="Follow">
      </form>"#,
      target_user = escape_html(&viewed_username)
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
      target_user = escape_html(&viewed_username)
    )
  } else {
    String::new()
  };
  
  let follow_section_html = &format!(
      r#"<div id="follow-section">
      {follow_form_html}
      {unfollow_form_html}
    </div>"#,
      follow_form_html = follow_form_html,
      unfollow_form_html = unfollow_form_html
    );

  let show_edit_profile_link = session_username
    .as_ref()
    .map(|username| username.eq_ignore_ascii_case(&viewed_username))
    .unwrap_or(false);

  let edit_profile_link = if show_edit_profile_link {
    format!(r#"<p><a href="https://{DOMAIN}/v1/editprofile?ib_uid={ib_uid}&ib_user={ib_user}">:[[ :edit-profile: ]]:</a></p><br>"#, ib_uid = ib_uid, ib_user = escape_html(ib_user))
  } else {
    String::new()
  };

  let ib_pro_result = sqlx::query_as::<_, ProRow>(
      "SELECT ibp, pro, location, services, website, github FROM pro WHERE ib_uid = ?"
    )
    .bind(ib_uid)
    .fetch_optional(&state.db_pool)
    .await
    .map(|opt| opt.unwrap_or_default());

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

  if let Ok(ib_pro) = ib_pro_result {

  let ib_post_results_length: i64 = sqlx::query_scalar(
      "SELECT COUNT(*) FROM post WHERE post.ib_uid = ? AND (post.parentid = \"\" OR post.parentid IS NULL)"
    )
    .bind(ib_uid)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

  let ib_post_results = sqlx::query_as::<_, PostRow>(
      "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks, user.pinned_postid FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE post.ib_uid = ? AND (post.parentid = \"\" OR post.parentid IS NULL) ORDER BY (post.postid = user.pinned_postid) DESC, post.timestamp DESC LIMIT 21"
    )
    .bind(ib_uid)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| format!("Post query failed: {}", e))?;

  let profile_has_more = ib_post_results.len() > 20;
  let display_rows = &ib_post_results[..ib_post_results.len().min(20)];

  let mut selected_user_posts_response_content = format!(
    r#"<br><div class="notice"><p><em>:[[ :for-the: [[ posts: is-by: {ib_post_results_length}: is-with: showing-latest-results: ]]:</em></p></div>"#,
    ib_post_results_length = ib_post_results_length,
  );

  let displayed_post_ids: Vec<String> = display_rows
    .iter()
    .map(|row| row.postid.clone())
    .collect();
  let acknowledged_post_ids = acknowledged_post_ids_for_user(&state.db_pool, session_uid, &displayed_post_ids).await;

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
          <a href="javascript:void(0);" class="edit-post">:[[ :edit: ]]:</a>&nbsp;&nbsp;&nbsp;&nbsp;<a href="javascript:void(0);" class="delete-post">:[[ :delete: ]]:</a>&nbsp;&nbsp;&nbsp;&nbsp;<a href="javascript:void(0);" class="pin-post-link">:[[ :pin-post: ]]:</a>&nbsp;&nbsp;&nbsp;&nbsp;"#,
        ib_uid = ib_uid,
        ib_user = escape_html(ib_user),
        ib_post_id = escape_html(&row.postid),
      )
    } else {
      String::new()
    };

    selected_user_posts_response_content += &format!(
      r#"
      {pinned_label}
      <div class="post" data-postid="{ib_post_id}" data-timestamp="{ib_post_timestamp}">
        {post_meta}
        <div class="post-content">{post_body}</div>
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
      pinned_label = if row.pinned_postid.as_deref() == Some(&row.postid) { "<div class=\"pinned-label\" style=\"font-weight: bold; margin-bottom: -10px; color: #AFAFAF;\">📌 Pinned</div>" } else { "" },
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

  let sentinel_html = if profile_has_more {
    r#"<div id="posts-load-sentinel"></div>"#
  } else {
    ""
  };

  let selected_user_posts_section = &format!(
    r#"<div id="selected-user-posts-section" data-ib-uid="{ib_uid}" data-ib-user="{ib_user_escaped}">{selected_user_posts_response_content}{sentinel_html}</div>"#,
    selected_user_posts_response_content = selected_user_posts_response_content,
    ib_uid = ib_uid,
    ib_user_escaped = escape_html(ib_user),
    sentinel_html = sentinel_html,
  );

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

  let github_identity_html = render_github_identity_html(state, ib_user).await;
  let sidebar_login_html = if session_uid.is_none() {
    r#"<div id="actions-section">
    <div class="login-section">
      <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
    </div>
  </div>"#
      .to_string()
  } else {
    String::new()
  };

  let viewed_ib_uid = ib_uid;
  let viewed_ib_user = escape_html(ib_user);
  let ib_user_escaped = escape_html(ib_user);
  let ib_ibp_escaped = escape_html(&ib_pro.ibp);
  let ib_pro_escaped = escape_html(&ib_pro.pro);
  let ib_services_escaped = escape_html(&ib_pro.services);
  let ib_location_escaped = escape_html(&ib_pro.location);
  let ib_website_escaped = escape_html(&ib_pro.website);
  let related_users_html = related_users(state, session_uid).await;
  let trending_tags_html = render_trending_tags_html(state, ib_uid, ib_user).await;
  
  context.insert("ib_user", &ib_user_escaped);
  context.insert("ib_uid", &ib_uid);
  context.insert("viewed_ib_uid", &viewed_ib_uid);
  context.insert("viewed_ib_user", &viewed_ib_user);
  context.insert("domain", &DOMAIN);
  context.insert("navigation_links", &navigation_links);
  context.insert("github_identity_html", &github_identity_html);
  context.insert("sidebar_login_html", &sidebar_login_html);
  context.insert("advert_html", &advert_html);
  context.insert("rank_name", &rank_name);
  context.insert("rank_level", &rank_level);
  context.insert("ib_ibp", &ib_ibp_escaped);
  context.insert("ib_pro", &ib_pro_escaped);
  context.insert("ib_services", &ib_services_escaped);
  context.insert("ib_location", &ib_location_escaped);
  context.insert("ib_website", &ib_website_escaped);
  context.insert("follow_section_html", &follow_section_html);
  context.insert("edit_profile_link", &edit_profile_link);
  context.insert("selected_user_posts_section", selected_user_posts_section);
  context.insert("related_users", &related_users_html);
  context.insert("trending_tags", &trending_tags_html);
  context.insert("copyright", &COPYRIGHT);
  
  let html = TEMPLATES.render("profile.html", &context)
        .map_err(|e| {
            use std::error::Error;
            let mut err_msg = format!("Template error: {}", e);
            let mut cause = e.source();
            while let Some(err) = cause {
                err_msg.push_str(&format!("\nCaused by: {}", err));
                cause = err.source();
            }
            err_msg
        })?;

    Ok(html)
  }

  else {
    Err("ib_pro_result failed".to_string())
  }
}

pub async fn render_profile_mobile_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

  let viewed_user_row = sqlx::query_as::<_, FollowLookupRow>(
      "SELECT username, COALESCE(followers, '') AS followers FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1"
    )
    .bind(ib_uid.to_string())
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| format!("Viewed user lookup failed: {}", e))?;

  let viewed_username = viewed_user_row
    .as_ref()
    .map(|row| row.username.clone())
    .unwrap_or_else(|| ib_user.to_string());

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

  let follower_list: Vec<String> = viewed_user_row
    .as_ref()
    .map(|row| {
      row
        .followers
        .split(',')
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
    })
    .unwrap_or_default();

  let show_edit_profile_link = session_username
    .as_ref()
    .map(|username| username.eq_ignore_ascii_case(&viewed_username))
    .unwrap_or(false);

  let edit_profile_link = if show_edit_profile_link {
    format!(r#"<p><a href="https://{DOMAIN}/v1/editprofile?ib_uid={ib_uid}&ib_user={ib_user}">:[[ :edit-profile: ]]:</a></p><br>"#, ib_uid = ib_uid, ib_user = escape_html(ib_user))
  } else {
    String::new()
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

  let ib_pro_result = sqlx::query_as::<_, ProRow>(
      "SELECT ibp, pro, location, services, website, github FROM pro WHERE ib_uid = ?"
    )
    .bind(ib_uid)
    .fetch_optional(&state.db_pool)
    .await
    .map(|opt| opt.unwrap_or_default());

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
  let github_identity_html = render_github_identity_html(state, ib_user).await;

  let ib_post_results_length: i64 = sqlx::query_scalar(
      "SELECT COUNT(*) FROM post WHERE post.ib_uid = ? AND (post.parentid = \"\" OR post.parentid IS NULL)"
    )
    .bind(ib_uid)
    .fetch_one(&state.db_pool)
    .await
    .unwrap_or(0);

  let ib_post_results = sqlx::query_as::<_, PostRow>(
      "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks, user.pinned_postid FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE post.ib_uid = ? AND (post.parentid = \"\" OR post.parentid IS NULL) ORDER BY (post.postid = user.pinned_postid) DESC, post.timestamp DESC LIMIT 21"
    )
    .bind(ib_uid)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| format!("Post query failed: {}", e))?;

  let profile_has_more = ib_post_results.len() > 20;
  let display_rows = &ib_post_results[..ib_post_results.len().min(20)];

  let mut selected_user_posts_response_content = format!(
    r#"<br><div class="notice"><p><em>:[[ :for-the: [[ posts: is-by: {ib_post_results_length}: is-with: showing-latest-results: ]]:</em></p></div>"#,
    ib_post_results_length = ib_post_results_length,
  );

  let displayed_post_ids: Vec<String> = display_rows
    .iter()
    .map(|row| row.postid.clone())
    .collect();
  let acknowledged_post_ids = acknowledged_post_ids_for_user(&state.db_pool, session_uid, &displayed_post_ids).await;

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
          <a href="javascript:void(0);" class="edit-post">:[[ :edit: ]]:</a>&nbsp;&nbsp;&nbsp;&nbsp;<a href="javascript:void(0);" class="delete-post">:[[ :delete: ]]:</a>&nbsp;&nbsp;&nbsp;&nbsp;<a href="javascript:void(0);" class="pin-post-link">:[[ :pin-post: ]]:</a>&nbsp;&nbsp;&nbsp;&nbsp;"#,
        ib_uid = ib_uid,
        ib_user = escape_html(ib_user),
        ib_post_id = escape_html(&row.postid),
      )
    } else {
      String::new()
    };

    selected_user_posts_response_content += &format!(
      r#"
      {pinned_label}
      <div class="post" data-postid="{ib_post_id}" data-timestamp="{ib_post_timestamp}">
        {post_meta}
        <div class="post-content">{post_body}</div>
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
      pinned_label = if row.pinned_postid.as_deref() == Some(&row.postid) { "<div class=\"pinned-label\" style=\"font-weight: bold; margin-bottom: -10px; color: #AFAFAF;\">📌 Pinned</div>" } else { "" },
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

  let sentinel_html = if profile_has_more {
    r#"<div id="posts-load-sentinel"></div>"#
  } else {
    ""
  };

  let selected_user_posts_html = &format!(
    r#"<div id="selected-user-posts-section" data-ib-uid="{ib_uid}" data-ib-user="{ib_user_escaped}">{selected_user_posts_response_content}{sentinel_html}</div>"#,
    selected_user_posts_response_content = selected_user_posts_response_content,
    ib_uid = ib_uid,
    ib_user_escaped = escape_html(ib_user),
    sentinel_html = sentinel_html,
  );

  let (ib_ibp, ib_pro_text, ib_services, ib_location, ib_website) = match ib_pro_result {
    Ok(pro) => (
      escape_html(&pro.ibp),
      escape_html(&pro.pro),
      escape_html(&pro.services),
      escape_html(&pro.location),
      escape_html(&pro.website),
    ),
    Err(_) => (String::new(), String::new(), String::new(), String::new(), String::new()),
  };

  let source_uid = session_uid.unwrap_or(ib_uid);
  let source_profile_terms = if let Some(uid) = session_uid {
    lookup_profile_terms_by_uid(state, uid)
      .await
      .unwrap_or_else(|| format!("{} {}", ib_pro_text, ib_ibp))
  } else {
    format!("{} {}", ib_pro_text, ib_ibp)
  };
  let related_userlist_html =
    render_related_userlist_html(state, session_uid, source_uid, &source_profile_terms).await;
  let trending_tags_html = render_trending_tags_html(state, ib_uid, ib_user).await;

  let profile_html = &format!(r#"
    <p><strong>{github_identity_html}</strong></p>
    <p class="description">{rank_name} - {rank_level}</p>
    <p class="paragraph"><em>{ib_ibp}</em></p>
    <p class="description">{ib_pro}</p>
    <p class="description">{ib_services}</p>
    <p class="description">{ib_location}</p>
    <p><a target="_blank" rel="noopener" href="{ib_website}">{ib_website}</a></p><br>
    {edit_profile_link}
    <p><a class="projects-display" href="https://{DOMAIN}/v1/projects?ib_uid={viewed_ib_uid}&ib_user={viewed_ib_user}">:[[ :projects: ]]:</a></p><br>
    <div id="related-userlist-section">
      {related_userlist_html}
    </div>
    <p>&nbsp;</p>
    <div id="trending-tags-section">
      {trending_tags_html}
    </div>
    <p>&nbsp;</p>"#,
    ib_ibp = ib_ibp,
    ib_pro = ib_pro_text,
    ib_services = ib_services,
    ib_location = ib_location,
    ib_website = ib_website,
    viewed_ib_uid = ib_uid,
    viewed_ib_user = escape_html(ib_user),
    edit_profile_link = edit_profile_link,
    github_identity_html = github_identity_html,
    rank_name = rank_name,
    rank_level = rank_level,
    related_userlist_html = related_userlist_html,
    trending_tags_html = trending_tags_html,
  );

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
    .map(|username| !show_unfollow && !username.eq_ignore_ascii_case(&viewed_username))
    .unwrap_or(false);

  let follow_form_html = if show_follow {
    format!(
      r#"<form id="follow-form" action="https://{DOMAIN}/v1/follow" method="POST">
        <input type="hidden" name="target_user" value="{target_user}">
        <input type="submit" value="Follow">
      </form>"#,
      target_user = escape_html(&viewed_username)
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
      target_user = escape_html(&viewed_username)
    )
  } else {
    String::new()
  };

  let follow_section_html = &format!(
      r#"<div id="follow-section">
      {follow_form_html}
      {unfollow_form_html}
    </div>"#,
      follow_form_html = follow_form_html,
      unfollow_form_html = unfollow_form_html
    );

  let session_nav_uid = session_uid.unwrap_or(ib_uid);
  let session_nav_user = session_username.as_deref().unwrap_or(ib_user);
  let navigation_links = &if session_uid.is_some() {
    format!(
      r#"<nav class="bottom-nav pwa-nav force-horizontal-nav">
    <a class="post-form-display" href="javascript:void(0);">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
          <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
        </svg>
      </div>
    </a>
    <a class="pro-home-display" href="https://{DOMAIN}/v1/profile/{session_ib_user}">
      <div class="nav-icon">
        <img src="https://github.com/{session_ib_user}.png?size=64" alt="Profile"
          style="width: 32px; height: 32px; border-radius: 50%;">
      </div>
    </a>
    <a class="search-display"
      href="https://{DOMAIN}/v1/search-section?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
      </div>
    </a>
    <a class="war-room-display"
      href="https://{DOMAIN}/v1/warroom?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <circle cx="12" cy="12" r="4"></circle>
          <line x1="12" y1="2" x2="12" y2="8"></line>
          <line x1="12" y1="16" x2="12" y2="22"></line>
          <line x1="2" y1="12" x2="8" y2="12"></line>
          <line x1="16" y1="12" x2="22" y2="12"></line>
        </svg>
      </div>
    </a>
    <a class="projects-display"
      href="https://{DOMAIN}/v1/projects?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
        </svg>
      </div>
    </a>
    <a class="dm-inbox-display" href="https://{DOMAIN}/v1/inbox?ib_uid={session_ib_uid}&ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
          style="vertical-align: middle; margin-right: 4px;">
          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
          <polyline points="22,6 12,13 2,6"></polyline>
        </svg> <span id="dm-unread-count">{unread_dm_count}</span>
      </div>
    </a>
  </nav>"#,
      session_ib_uid = session_nav_uid,
      session_ib_user = escape_html(session_nav_user),
    )
  } else {
      r#"<div id="actions-section">
      <div class="login-section">
        <p><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#
        .to_string()
  };

  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &escape_html(ib_user));
  context.insert("domain", &DOMAIN);
  context.insert("advert_html", &advert_html);
  context.insert("profile_html", &profile_html);
  context.insert("follow_section_html", &follow_section_html);
  context.insert("sentinel_html", &sentinel_html);
  context.insert("selected_user_posts_html", &selected_user_posts_html);
  context.insert("navigation_links", &navigation_links);
  
  let html = TEMPLATES.render("profile_mobile.html", &context)
    .map_err(|e| {
        use std::error::Error;
        let mut err_msg = format!("Template error: {}", e);
        let mut cause = e.source();
        while let Some(err) = cause {
            err_msg.push_str(&format!("\nCaused by: {}", err));
            cause = err.source();
        }
        err_msg
    })?;

  Ok(html)
}

pub async fn render_search_users_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  raw_query: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

  let search_terms: Vec<String> = raw_query
    .split_whitespace()
    .map(|term| term.trim().to_lowercase())
    .filter(|term| !term.is_empty())
    .collect();

  let search_results_html = if raw_query.trim().is_empty() {
    "<p><em>:[[ :search-users: empty-query: ]]:</em></p>".to_string()
  } else {
    let sql = "SELECT CAST(COALESCE(CONVERT(user.username USING utf8mb4), '') AS CHAR CHARACTER SET utf8mb4) AS username, COALESCE(user.total_acknowledgments, 0) AS total_acknowledgments, COALESCE(candidate.ibp, '') AS ibp FROM pro AS candidate LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(candidate.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE MATCH(user.username) AGAINST(? IN BOOLEAN MODE) OR MATCH(candidate.ibp, candidate.pro, candidate.services, candidate.location, candidate.website, candidate.github) AGAINST(? IN BOOLEAN MODE) ORDER BY RAND() LIMIT 200";

    // Transform raw_query to add '*' to each word for prefix matching
    let boolean_query = raw_query.split_whitespace().map(|w| format!("{}*", w)).collect::<Vec<_>>().join(" ");

    let rows = sqlx::query_as::<_, SearchUserRow>(sql)
      .bind(&boolean_query)
      .bind(&boolean_query)
      .fetch_all(&state.db_pool)
      .await
      .map_err(|e| format!("Search users query failed: {}", e))?;

    let mut html = String::new();

    for row in rows {
      if row.username.trim().is_empty() {
        continue;
      }

      let profile_link = render_project_profile_link(&row.username, row.total_acknowledgments);

      html += &format!(
        r#"<div class="user-search-result-section">
          <p>{profile_link}<br><small>{ibp}</small></p>
        </div>"#,
        profile_link = profile_link,
        ibp = highlight_terms(&row.ibp, &search_terms)
      );
    }

    if html.is_empty() {
      r#"<div class="notice"><p><em>:[[ :search-users: is-by: no: is-with: results: ]]:</em></p></div>"#.to_string()
    } else {
      html
    }
  };

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

  let search_section_html = format!(
    r#"<div id="selected-user-posts-section" class="post-section">
        <div class="notice"><p><em>:[[ :search-users: for-the: {raw_query}: ]]:</em></p></div>
        {search_results_html}
      </div>"#,
    raw_query = escape_html(raw_query),
    search_results_html = search_results_html
  );

  let ib_pro_result = sqlx::query_as::<_, ProRow>(
      "SELECT ibp, pro, location, services, website, github FROM pro WHERE ib_uid = ?"
    )
    .bind(ib_uid)
    .fetch_optional(&state.db_pool)
    .await
    .map(|opt| opt.unwrap_or_default());

  let ib_pro = ib_pro_result.unwrap_or_else(|_| ProRow {
    ibp: String::new(),
    pro: String::new(),
    location: String::new(),
    services: String::new(),
    website: String::new(),
  });
  
  let github_identity_html = render_github_identity_html(state, ib_user).await;
  let sidebar_login_html = if session_uid.is_none() {
    r#"<div id="actions-section">
    <div class="login-section">
      <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
    </div>
  </div>"#
      .to_string()
  } else {
    String::new()
  };

  let viewed_user_row = match sqlx::query_as::<_, UserHoverLookupRow>(
    "SELECT CONVERT(ib_uid USING utf8mb4) AS ib_uid, username, COALESCE(followers, '') AS followers, COALESCE(total_acknowledgments, 0) AS total_acknowledgments FROM user WHERE LOWER(username) = LOWER(?) LIMIT 1",
  )
  .bind(ib_user)
  .fetch_optional(&state.db_pool)
  .await
  {
    Ok(Some(row)) => Some(row),
    _ => None,
  };
  
  let total_acks = viewed_user_row.as_ref().map(|row| row.total_acknowledgments).unwrap_or(0);
  let (rank_level, rank_name) = rank_from_unique_acknowledgments(total_acks);


  let mut follower_list: Vec<String> = Vec::new();
  if let Some(row) = &viewed_user_row {
    follower_list = row
      .followers
      .split(',')
      .map(|s| s.trim().to_string())
      .filter(|s| !s.is_empty())
      .collect();
  }

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
  
  let follow_section_html = &format!(
      r#"<div id="follow-section">
      {follow_form_html}
      {unfollow_form_html}
    </div>"#,
      follow_form_html = follow_form_html,
      unfollow_form_html = unfollow_form_html
    );

  let show_edit_profile_link = session_username
    .as_ref()
    .map(|u| u.eq_ignore_ascii_case(ib_user))
    .unwrap_or(false);

  let edit_profile_link = if show_edit_profile_link {
    format!(r#"<p><a href="https://{DOMAIN}/v1/editprofile?ib_uid={ib_uid}&ib_user={ib_user}">:[[ :edit-profile: ]]:</a></p><br>"#, ib_uid = ib_uid, ib_user = escape_html(ib_user))
  } else {
    String::new()
  };

  let viewed_ib_uid = ib_uid;
  let viewed_ib_user = escape_html(ib_user);
  let ib_user_escaped = escape_html(ib_user);
  let ib_ibp_escaped = escape_html(&ib_pro.ibp);
  let ib_pro_escaped = escape_html(&ib_pro.pro);
  let ib_services_escaped = escape_html(&ib_pro.services);
  let ib_location_escaped = escape_html(&ib_pro.location);
  let ib_website_escaped = escape_html(&ib_pro.website);
  let related_users_html = related_users(state, session_uid).await;
  let trending_tags_html = render_trending_tags_html(state, ib_uid, ib_user).await;
  
  context.insert("ib_user", &ib_user_escaped);
  context.insert("ib_uid", &ib_uid);
  context.insert("viewed_ib_uid", &viewed_ib_uid);
  context.insert("viewed_ib_user", &viewed_ib_user);
  context.insert("domain", &DOMAIN);
  context.insert("navigation_links", &navigation_links);
  context.insert("github_identity_html", &github_identity_html);
  context.insert("sidebar_login_html", &sidebar_login_html);
  context.insert("advert_html", &advert_html);
  context.insert("rank_name", &rank_name);
  context.insert("rank_level", &rank_level);
  context.insert("ib_ibp", &ib_ibp_escaped);
  context.insert("ib_pro", &ib_pro_escaped);
  context.insert("ib_services", &ib_services_escaped);
  context.insert("ib_location", &ib_location_escaped);
  context.insert("ib_website", &ib_website_escaped);
  context.insert("follow_section_html", &follow_section_html);
  context.insert("edit_profile_link", &edit_profile_link);
  context.insert("search_section_html", &search_section_html);
  context.insert("related_users", &related_users_html);
  context.insert("trending_tags", &trending_tags_html);
  context.insert("copyright", &COPYRIGHT);
  
  let html = TEMPLATES.render("user_search_section.html", &context)
  .map_err(|e| {
      use std::error::Error;
      let mut err_msg = format!("Template error: {}", e);
      let mut cause = e.source();
      while let Some(err) = cause {
          err_msg.push_str(&format!("\nCaused by: {}", err));
          cause = err.source();
      }
      err_msg
  })?;

  Ok(html)
}

pub async fn render_search_users_mobile_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  raw_query: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

  let mut search_results_section_html = String::new();
  let search_terms: Vec<String> = raw_query
    .split_whitespace()
    .map(|term| term.trim().to_lowercase())
    .filter(|term| !term.is_empty())
    .collect();

  let search_results_html = if raw_query.trim().is_empty() {
    "<p><em>:[[ :search-users: empty-query: ]]:</em></p>".to_string()
  } else {
    let sql = "SELECT CAST(COALESCE(CONVERT(user.username USING utf8mb4), '') AS CHAR CHARACTER SET utf8mb4) AS username, COALESCE(user.total_acknowledgments, 0) AS total_acknowledgments, COALESCE(candidate.ibp, '') AS ibp FROM pro AS candidate LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(candidate.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE MATCH(user.username) AGAINST(? IN BOOLEAN MODE) OR MATCH(candidate.ibp, candidate.pro, candidate.services, candidate.location, candidate.website, candidate.github) AGAINST(? IN BOOLEAN MODE) ORDER BY RAND() LIMIT 200";

    // Transform raw_query to add '*' to each word for prefix matching
    let boolean_query = raw_query.split_whitespace().map(|w| format!("{}*", w)).collect::<Vec<_>>().join(" ");

    let rows = sqlx::query_as::<_, SearchUserRow>(sql)
      .bind(&boolean_query)
      .bind(&boolean_query)
      .fetch_all(&state.db_pool)
      .await
      .map_err(|e| format!("Search users query failed: {}", e))?;

    let mut html = String::new();

    for row in rows {
      if row.username.trim().is_empty() {
        continue;
      }

      let profile_link = render_project_profile_link(&row.username, row.total_acknowledgments);

      html += &format!(
        r#"<div class="user-search-result-section">
          <p>{profile_link}<br><small>{ibp}</small></p>
        </div>"#,
        profile_link = profile_link,
        ibp = highlight_terms(&row.ibp, &search_terms)
      );
    }

    if html.is_empty() {
      r#"<div class="notice"><p><em>:[[ :search-users: is-by: no: is-with: results: ]]:</em></p></div>"#.to_string()
    } else {
      html
    }
  };

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

  search_results_section_html += &format!(
    r#"<div class="notice"><p><em>:[[ :search-users: for-the: {raw_query}: ]]:</em></p></div>
      {search_results_html}"#,
    raw_query = escape_html(raw_query),
    search_results_html = search_results_html,
  );

  let unread_dm_count = if let Some(uid) = session_uid {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM dm WHERE recipient_uid = ? AND read_at IS NULL")
      .bind(uid)
      .fetch_one(&state.db_pool)
      .await
      .unwrap_or(0)
  } else {
    0
  };

  let session_nav_uid = session_uid.unwrap_or(ib_uid);
  let session_nav_user = session_username.as_deref().unwrap_or(ib_user);
  let navigation_links = &if session_uid.is_some() {
    format!(
      r#"<nav class="bottom-nav pwa-nav force-horizontal-nav">
    <a class="post-form-display" href="javascript:void(0);">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
          <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
        </svg>
      </div>
    </a>
    <a class="pro-home-display" href="https://{DOMAIN}/v1/profile/{session_ib_user}">
      <div class="nav-icon">
        <img src="https://github.com/{session_ib_user}.png?size=64" alt="Profile"
          style="width: 32px; height: 32px; border-radius: 50%;">
      </div>
    </a>
    <a class="search-display"
      href="https://{DOMAIN}/v1/search-section?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
      </div>
    </a>
    <a class="war-room-display"
      href="https://{DOMAIN}/v1/warroom?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <circle cx="12" cy="12" r="4"></circle>
          <line x1="12" y1="2" x2="12" y2="8"></line>
          <line x1="12" y1="16" x2="12" y2="22"></line>
          <line x1="2" y1="12" x2="8" y2="12"></line>
          <line x1="16" y1="12" x2="22" y2="12"></line>
        </svg>
      </div>
    </a>
    <a class="projects-display"
      href="https://{DOMAIN}/v1/projects?ib_uid={viewed_ib_uid}&amp;ib_user={viewed_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
        </svg>
      </div>
    </a>
    <a class="dm-inbox-display" href="https://{DOMAIN}/v1/inbox?ib_uid={session_ib_uid}&ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
          style="vertical-align: middle; margin-right: 4px;">
          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
          <polyline points="22,6 12,13 2,6"></polyline>
        </svg> <span id="dm-unread-count">{unread_dm_count}</span>
      </div>
    </a>
  </nav>"#,
      session_ib_uid = session_nav_uid,
      session_ib_user = escape_html(session_nav_user),
      viewed_ib_uid = ib_uid,
      viewed_ib_user = escape_html(ib_user),
    )
  } else {
      r#"<div id="actions-section">
      <div class="login-section">
        <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#
        .to_string()
  };

  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &ib_user);
  context.insert("domain", &DOMAIN);
  context.insert("advert_html", &advert_html);
  context.insert("search_results_section_html", &search_results_section_html);
  context.insert("navigation_links", &navigation_links);
  
  let html = TEMPLATES.render("user_search_results_mobile.html", &context)
    .map_err(|e| {
        use std::error::Error;
        let mut err_msg = format!("Template error: {}", e);
        let mut cause = e.source();
        while let Some(err) = cause {
            err_msg.push_str(&format!("\nCaused by: {}", err));
            cause = err.source();
        }
        err_msg
    })?;

  Ok(html)
}

pub async fn render_search_posts_mobile_html(
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

  if let Some(tag) = normalized_tag.clone() {
    let rows = sqlx::query_as::<_, PostRow>(
      "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks, user.pinned_postid FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci JOIN post_tag pt ON pt.postid = post.postid WHERE pt.tag = ? ORDER BY post.timestamp DESC LIMIT 200"
      )
      .bind(&tag)
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
            <div class="post-content">{post_body}</div>
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

  let session_nav_uid = session_uid.unwrap_or(ib_uid);
  let session_nav_user = session_username.as_deref().unwrap_or(ib_user);
  let navigation_links = &if session_uid.is_some() {
    format!(
      r#"<nav class="bottom-nav pwa-nav force-horizontal-nav">
    <a class="post-form-display" href="javascript:void(0);">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
          <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
        </svg>
      </div>
    </a>
    <a class="pro-home-display" href="https://{DOMAIN}/v1/profile/{session_ib_user}">
      <div class="nav-icon">
        <img src="https://github.com/{session_ib_user}.png?size=64" alt="Profile"
          style="width: 32px; height: 32px; border-radius: 50%;">
      </div>
    </a>
    <a class="search-display"
      href="https://{DOMAIN}/v1/search-section?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
      </div>
    </a>
    <a class="war-room-display"
      href="https://{DOMAIN}/v1/warroom?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <circle cx="12" cy="12" r="4"></circle>
          <line x1="12" y1="2" x2="12" y2="8"></line>
          <line x1="12" y1="16" x2="12" y2="22"></line>
          <line x1="2" y1="12" x2="8" y2="12"></line>
          <line x1="16" y1="12" x2="22" y2="12"></line>
        </svg>
      </div>
    </a>
    <a class="projects-display"
      href="https://{DOMAIN}/v1/projects?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
        </svg>
      </div>
    </a>
    <a class="dm-inbox-display" href="https://{DOMAIN}/v1/inbox?ib_uid={session_ib_uid}&ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
          style="vertical-align: middle; margin-right: 4px;">
          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
          <polyline points="22,6 12,13 2,6"></polyline>
        </svg> <span id="dm-unread-count">{unread_dm_count}</span>
      </div>
    </a>
  </nav>"#,
      session_ib_uid = session_nav_uid,
      session_ib_user = escape_html(session_nav_user),
    )
  } else {
      r#"<div id="actions-section">
      <div class="login-section">
        <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#
        .to_string()
  };

  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &ib_user);
  context.insert("domain", &DOMAIN);
  context.insert("advert_html", &advert_html);
  context.insert("tag", &tag);
  context.insert("post_html", &post_html);
  context.insert("navigation_links", &navigation_links);
  
  let html = TEMPLATES.render("search_posts_mobile.html", &context)
    .map_err(|e| {
        use std::error::Error;
        let mut err_msg = format!("Template error: {}", e);
        let mut cause = e.source();
        while let Some(err) = cause {
            err_msg.push_str(&format!("\nCaused by: {}", err));
            cause = err.source();
        }
        err_msg
    })?;

  Ok(html)
}

pub async fn render_search_posts_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  raw_tag: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

  let normalized_tag = normalize_hashtag(raw_tag);

  let search_results_html = if let Some(tag) = normalized_tag.clone() {
    let rows = sqlx::query_as::<_, PostRow>(
      "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks, user.pinned_postid FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci JOIN post_tag pt ON pt.postid = post.postid WHERE pt.tag = ? ORDER BY post.timestamp DESC LIMIT 200"
      )
      .bind(&tag)
      .fetch_all(&state.db_pool)
      .await
      .map_err(|e| format!("Search posts query failed: {}", e))?;

    if rows.is_empty() {
      "<p><em>:[[ :search-posts: is-by: no: is-with: results: ]]:</em></p>".to_string()
    } else {
      let mut post_html = String::new();
      let row_post_ids: Vec<String> = rows.iter().map(|row| row.postid.clone()).collect();
      let acknowledged_post_ids = acknowledged_post_ids_for_user(&state.db_pool, session_uid, &row_post_ids).await;

      for row in rows {
        post_html += &format!(
          r#"<div class="post" data-postid="{post_id}" data-timestamp="{post_timestamp}">
            {post_meta}
            <div class="post-content">{post_body}</div>
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

      post_html
    }
  } else {
    "<p><em>:[[ :search-posts: invalid-tag: ]]:</em></p>".to_string()
  };

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

  let mut github_identity_html = String::new();
  let mut rank_name = String::new();
  let mut rank_level = 0;
  let mut ib_ibp_escaped = String::new();
  let mut ib_pro_str = String::new();
  let mut ib_services_escaped = String::new();
  let mut ib_location_escaped = String::new();
  let mut ib_website_escaped = String::new();
  let mut edit_profile_link = String::new();
  let follow_section_html = String::new();
  let mut related_users_html = String::new();
  let mut trending_tags_html = String::new();
  let mut sidebar_login_html = String::new();

  if session_uid.is_none() {
    sidebar_login_html = r#"<div id="actions-section">
      <div class="login-section">
        <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#.to_string();
  }

  let ib_pro_result = sqlx::query_as::<_, ProRow>(
      "SELECT ibp, pro, location, services, website, github FROM pro WHERE ib_uid = ?"
    )
    .bind(ib_uid)
    .fetch_optional(&state.db_pool)
    .await
    .map(|opt| opt.unwrap_or_default());

  if let Ok(ib_pro) = ib_pro_result {
    ib_ibp_escaped = escape_html(&ib_pro.ibp);
    ib_pro_str = escape_html(&ib_pro.pro);
    ib_services_escaped = escape_html(&ib_pro.services);
    ib_location_escaped = escape_html(&ib_pro.location);
    ib_website_escaped = escape_html(&ib_pro.website);

    let source_uid = session_uid.unwrap_or(ib_uid);
    let source_profile_terms = if let Some(uid) = session_uid {
      lookup_profile_terms_by_uid(state, uid)
        .await
        .unwrap_or_else(|| format!("{} {}", ib_pro.pro, ib_pro.ibp))
    } else {
      format!("{} {}", ib_pro.pro, ib_pro.ibp)
    };
    related_users_html =
      render_related_userlist_html(state, session_uid, source_uid, &source_profile_terms).await;
    trending_tags_html = render_trending_tags_html(state, ib_uid, ib_user).await;
    github_identity_html = render_github_identity_html(state, ib_user).await;

    let total_acks = sqlx::query_scalar::<_, i64>("SELECT total_acknowledgments FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ?")
      .bind(ib_uid.to_string())
      .fetch_optional(&state.db_pool)
      .await
      .unwrap_or(Some(0))
      .unwrap_or(0);
    let rank_info = get_rank_info(total_acks);
    rank_name = rank_info.name.to_string();
    rank_level = rank_info.level;

    if session_uid == Some(ib_uid) {
        edit_profile_link = r#"<p><a class="edit-profile" href="javascript:void(0);">:[[ :edit-profile: ]]:</a></p>"#.to_string();
    }
  }

  context.insert("search_results_html", &search_results_html);
  context.insert("navigation_links", &navigation_links);
  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &ib_user);
  context.insert("tag", raw_tag);
  context.insert("advert_html", &advert_html);
  context.insert("sidebar_login_html", &sidebar_login_html);
  context.insert("related_users", &related_users_html);
  context.insert("trending_tags", &trending_tags_html);
  context.insert("ib_pro", &ib_pro_str);
  context.insert("ib_services", &ib_services_escaped);
  context.insert("ib_location", &ib_location_escaped);
  context.insert("ib_website", &ib_website_escaped);
  context.insert("ib_ibp", &ib_ibp_escaped);
  context.insert("github_identity_html", &github_identity_html);
  context.insert("rank_name", &rank_name);
  context.insert("rank_level", &rank_level);
  context.insert("edit_profile_link", &edit_profile_link);
  context.insert("follow_section_html", &follow_section_html);
  context.insert("domain", &DOMAIN);
  context.insert("viewed_ib_uid", &ib_uid);
  context.insert("viewed_ib_user", &ib_user);
  context.insert("copyright", &COPYRIGHT);

  let html = TEMPLATES.render("search_posts.html", &context)
    .map_err(|e| {
        use std::error::Error;
        let mut err_msg = format!("Template error: {}", e);
        let mut cause = e.source();
        while let Some(err) = cause {
            err_msg.push_str(&format!("\nCaused by: {}", err));
            cause = err.source();
        }
        err_msg
    })?;

  Ok(html)
}

pub async fn render_projects_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

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

  let rows = sqlx::query_as::<_, ProjectProfileRow>(
      "SELECT project.id, project.ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(project.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, COALESCE(user.total_acknowledgments, 0) AS total_acknowledgments, project.project, project.description, project.languages, CAST(project.updated_at AS CHAR CHARACTER SET utf8mb4) AS updated_at, COALESCE(project.reinforcements, \"\") AS reinforcements, COALESCE(project.reinforcements_request, FALSE) AS reinforcements_request FROM project_profile AS project LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(project.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE project.ib_uid = ? ORDER BY project.updated_at DESC LIMIT 500"
    )
    .bind(ib_uid)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| format!("Projects query failed: {}", e))?;

  let projects_html = if rows.is_empty() {
    r#"<br><div class="notice"><p><em>:[[ :is-by: none: for-the: user-projects: ]]:</em></p></div>"#.to_string()
  } else {
    let reinforcement_names: HashSet<String> = rows
      .iter()
      .flat_map(|row| {
        row
          .reinforcements
          .as_deref()
          .unwrap_or("")
          .split(',')
          .map(|item| item.trim())
          .filter(|item| !item.is_empty())
          .map(|item| item.to_string())
          .collect::<Vec<String>>()
      })
      .collect();
    let reinforcement_ack_map = load_project_profile_ack_map(state, &reinforcement_names).await;

    rows
      .iter()
      .map(|row| {
        let can_edit = session_uid == Some(row.ib_uid);
        let owner_link = render_project_profile_link(&row.username, row.total_acknowledgments);

        if can_edit {
          format!(
            r#"<br><div class="post">
              <div class="post-meta">{owner_link}<span class="post-timestamp">{updated_at}</span></div>
              <form class="edit-project-form" action="https://{DOMAIN}/v1/projects/edit" method="POST">
                <input type="hidden" name="ib_uid" value="{ib_uid}">
                <input type="hidden" name="ib_user" value="{ib_user}">
                <input type="hidden" name="project_id" value="{project_id}">
                <p><strong>Project:</strong></p>
                <input class="post" type="text" name="project" value="{project}" maxlength="255" required>
                <p><strong>Description:</strong></p>
                <input class="post" type="text" name="description" value="{description}" maxlength="1024" required>
                <p><strong>Languages:</strong></p>
                <input class="post" type="text" name="languages" value="{languages}" maxlength="255" required>
                <p><strong>Reinforcements:</strong></p>
                <input class="post" type="text" name="reinforcements" value="{reinforcements}" maxlength="9999">
                <p><strong>Requesting Reinforcements:</strong></p>
                <input type="checkbox" name="reinforcements_request" value="yes" {reinforcements_request_checked}>
                <input class="post-submit" type="submit" value="Save Project">
              </form>
            </div>"#,
            owner_link = owner_link,
            updated_at = escape_html(&row.updated_at),
            ib_uid = ib_uid,
            ib_user = escape_html(ib_user),
            project_id = row.id,
            project = escape_html(&row.project),
            description = escape_html(&row.description),
            languages = escape_html(&row.languages),
            reinforcements = escape_html(row.reinforcements.as_deref().unwrap_or("")),
            reinforcements_request_checked = if row.reinforcements_request == Some(true) { "checked" } else { "" }
          )
        } else {
          let reinforcements_badge = if row.reinforcements_request == Some(true) {
            "<p><em>:[[ :requesting-reinforcements: ]]:</em></p>".to_string()
          } else {
            String::new()
          };
          let reinforcement_usernames: Vec<String> = row
            .reinforcements
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
            .collect();
          let already_reinforcing = session_username
            .as_ref()
            .map(|username| {
              reinforcement_usernames
                .iter()
                .any(|name| name.eq_ignore_ascii_case(username))
            })
            .unwrap_or(false);

          let quick_response_form = if row.reinforcements_request == Some(true)
            && session_uid.is_some()
            && session_uid != Some(row.ib_uid)
            && !already_reinforcing
          {
            format!(
              r#"<form class="quick-response-force-form" action="https://{DOMAIN}/v1/projects/reinforce" method="POST">
                <input type="hidden" name="ib_uid" value="{ib_uid}">
                <input type="hidden" name="ib_user" value="{ib_user}">
                <input type="hidden" name="project_id" value="{project_id}">
                <input type="hidden" name="quick_response_force" value="1">
                <input class="post-submit quick-response-submit" type="submit" value="Respond to Reinforcements Request">
              </form>"#,
              ib_uid = ib_uid,
              ib_user = escape_html(ib_user),
              project_id = row.id
            )
          } else if row.reinforcements_request == Some(true)
            && session_uid.is_some()
            && session_uid != Some(row.ib_uid)
            && already_reinforcing
          {
            format!(
              r#"<form class="quick-response-force-form" action="https://{DOMAIN}/v1/projects/reinforce" method="POST">
                <input type="hidden" name="ib_uid" value="{ib_uid}">
                <input type="hidden" name="ib_user" value="{ib_user}">
                <input type="hidden" name="project_id" value="{project_id}">
                <input type="hidden" name="quick_response_force" value="retreat">
                <input class="post-submit quick-response-submit" type="submit" value="Retreat">
              </form>"#,
              ib_uid = ib_uid,
              ib_user = escape_html(ib_user),
              project_id = row.id
            )
          } else {
            String::new()
          };
          format!(
            r#"<br><div class="post">
              <div class="post-meta">{owner_link}<span class="post-timestamp">{updated_at}</span></div>
              <p><strong>Project:</strong> {project}</p>
              <p><strong>Description:</strong> {description}</p>
              <p><strong>Languages:</strong> {languages}</p>
              {reinforcements_section}
              {reinforcements_badge}
              {quick_response_form}
            </div>"#,
            owner_link = owner_link,
            updated_at = escape_html(&row.updated_at),
            project = escape_html(&row.project),
            description = render_post_with_hashtags(&row.description, ib_uid, ib_user),
            languages = escape_html(&row.languages),
            reinforcements_section = if let Some(ref r) = row.reinforcements {
              if !r.trim().is_empty() {
                let links: String = r.split(',').map(|name| name.trim()).filter(|name| !name.is_empty()).map(|name| {
                  let total_acks = reinforcement_ack_map
                    .get(&name.to_ascii_lowercase())
                    .copied()
                    .unwrap_or(0);
                  render_project_profile_link(name, total_acks)
                }).collect::<Vec<_>>().join(" ");
                format!("<p><strong>Reinforcements:</strong> {}</p>", links)
              } else {
                String::new()
              }
            } else {
              String::new()
            },
            reinforcements_badge = reinforcements_badge,
            quick_response_form = quick_response_form
          )
        }
      })
      .collect::<Vec<String>>()
      .join("")
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

  let add_project_form_html = if session_uid == Some(ib_uid) {
    format!(
      r#"<div class="notice"><p><strong>:[[ :add-project: ]]:</strong></p></div>
        <br><div class="post">
          <form id="add-project-form" action="https://{DOMAIN}/v1/projects" method="POST">
            <input type="hidden" name="ib_uid" value="{ib_uid}">
            <input type="hidden" name="ib_user" value="{ib_user}">
            <p><strong>Project:</strong></p>
            <input class="post" type="text" name="project" maxlength="255" required>
            <p><strong>Description:</strong></p>
            <input class="post" type="text" name="description" maxlength="1024" required>
            <p><strong>Languages:</strong></p>
            <input class="post" type="text" name="languages" maxlength="255" required>
            <input class="post-submit" type="submit" value="Add Project">
          </form>
        </div>"#,
      ib_uid = ib_uid,
      ib_user = escape_html(ib_user)
    )
  } else {
    String::new()
  };

  let ib_pro_result = sqlx::query_as::<_, ProRow>(
      "SELECT ibp, pro, location, services, website, github FROM pro WHERE ib_uid = ?"
    )
    .bind(ib_uid)
    .fetch_optional(&state.db_pool)
    .await
    .map(|opt| opt.unwrap_or_default());

  if let Ok(ib_pro) = ib_pro_result {
    let source_uid = session_uid.unwrap_or(ib_uid);
    let source_profile_terms = if let Some(uid) = session_uid {
      lookup_profile_terms_by_uid(state, uid)
        .await
        .unwrap_or_else(|| format!("{} {}", ib_pro.pro, ib_pro.ibp))
    } else {
      format!("{} {}", ib_pro.pro, ib_pro.ibp)
    };
    let related_userlist_html =
      render_related_userlist_html(state, session_uid, source_uid, &source_profile_terms).await;
    let trending_tags_html = render_trending_tags_html(state, ib_uid, ib_user).await;
    let github_identity_html = render_github_identity_html(state, ib_user).await;
    let sidebar_login_html = if session_uid.is_none() {
      r#"<div id="actions-section">
      <div class="login-section">
        <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#
        .to_string()
    } else {
      String::new()
    };

  let show_edit_profile_link = session_username
    .as_ref()
    .map(|u| u.eq_ignore_ascii_case(ib_user))
    .unwrap_or(false);

  let edit_profile_link = if show_edit_profile_link {
    format!(r#"<p><a href="https://{DOMAIN}/v1/editprofile?ib_uid={ib_uid}&ib_user={ib_user}">:[[ :edit-profile: ]]:</a></p><br>"#, ib_uid = ib_uid, ib_user = escape_html(ib_user))
  } else {
    String::new()
  };

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

  let viewed_ib_uid = ib_uid;
  let viewed_ib_user = escape_html(ib_user);
  let ib_user_escaped = escape_html(ib_user);
  let ib_ibp_escaped = escape_html(&ib_pro.ibp);
  let ib_pro_escaped = escape_html(&ib_pro.pro);
  let ib_services_escaped = escape_html(&ib_pro.services);
  let ib_location_escaped = escape_html(&ib_pro.location);
  let ib_website_escaped = escape_html(&ib_pro.website);

  let viewed_user_row = sqlx::query_as::<_, FollowLookupRow>(
      "SELECT username, COALESCE(followers, '') AS followers FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1"
    )
    .bind(ib_uid.to_string())
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| format!("Viewed user lookup failed: {}", e))?;

  let follower_list: Vec<String> = viewed_user_row
    .as_ref()
    .map(|row| {
      row
        .followers
        .split(',')
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
    })
    .unwrap_or_default();

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
    .map(|username| !show_unfollow && !username.eq_ignore_ascii_case(&viewed_ib_user))
    .unwrap_or(false);

  let follow_form_html = if show_follow {
    format!(
      r#"<form id="follow-form" action="https://{DOMAIN}/v1/follow" method="POST">
        <input type="hidden" name="target_user" value="{target_user}">
        <input type="submit" value="Follow">
      </form>"#,
      target_user = escape_html(&viewed_ib_user)
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
      target_user = escape_html(&viewed_ib_user)
    )
  } else {
    String::new()
  };
  
  let follow_section_html = &format!(
      r#"<div id="follow-section">
      {follow_form_html}
      {unfollow_form_html}
    </div>"#,
      follow_form_html = follow_form_html,
      unfollow_form_html = unfollow_form_html
    );

  context.insert("ib_user", &ib_user_escaped);
  context.insert("ib_uid", &ib_uid);
  context.insert("viewed_ib_uid", &viewed_ib_uid);
  context.insert("viewed_ib_user", &viewed_ib_user);
  context.insert("domain", &DOMAIN);
  context.insert("navigation_links", &navigation_links);
  context.insert("github_identity_html", &github_identity_html);
  context.insert("sidebar_login_html", &sidebar_login_html);
  context.insert("advert_html", &advert_html);
  context.insert("add_project_form_html", &add_project_form_html);
  context.insert("projects_html", &projects_html);
  context.insert("rank_name", &rank_name);
  context.insert("rank_level", &rank_level);
  context.insert("ib_ibp", &ib_ibp_escaped);
  context.insert("ib_pro", &ib_pro_escaped);
  context.insert("ib_services", &ib_services_escaped);
  context.insert("ib_location", &ib_location_escaped);
  context.insert("ib_website", &ib_website_escaped);
  context.insert("follow_section_html", &follow_section_html);
  context.insert("edit_profile_link", &edit_profile_link);
  context.insert("related_users", &related_userlist_html);
  context.insert("trending_tags", &trending_tags_html);
  context.insert("copyright", &COPYRIGHT);
  }

  let html = TEMPLATES.render("projects.html", &context)
    .map_err(|e| {
        use std::error::Error;
        let mut err_msg = format!("Template error: {}", e);
        let mut cause = e.source();
        while let Some(err) = cause {
            err_msg.push_str(&format!("\nCaused by: {}", err));
            cause = err.source();
        }
        err_msg
    })?;

  Ok(html)
}

pub async fn render_projects_mobile_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

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

  let rows = sqlx::query_as::<_, ProjectProfileRow>(
      "SELECT project.id, project.ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(project.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, COALESCE(user.total_acknowledgments, 0) AS total_acknowledgments, project.project, project.description, project.languages, CAST(project.updated_at AS CHAR CHARACTER SET utf8mb4) AS updated_at, COALESCE(project.reinforcements, '') AS reinforcements, COALESCE(project.reinforcements_request, FALSE) AS reinforcements_request FROM project_profile AS project LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(project.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE project.ib_uid = ? ORDER BY project.updated_at DESC LIMIT 500"
    )
    .bind(ib_uid)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| format!("Projects query failed: {}", e))?;

  let projects_html = if rows.is_empty() {
    r#"<br><div class="notice"><p><em>:[[ :no-projects-yet: ]]:</em></p></div>"#.to_string()
  } else {
    let reinforcement_names: HashSet<String> = rows
      .iter()
      .flat_map(|row| {
        row
          .reinforcements
          .as_deref()
          .unwrap_or("")
          .split(',')
          .map(|item| item.trim())
          .filter(|item| !item.is_empty())
          .map(|item| item.to_string())
          .collect::<Vec<String>>()
      })
      .collect();
    let reinforcement_ack_map = load_project_profile_ack_map(state, &reinforcement_names).await;

    rows
      .iter()
      .map(|row| {
        let can_edit = session_uid == Some(row.ib_uid);
        let owner_link = render_project_profile_link(&row.username, row.total_acknowledgments);

        if can_edit {
          format!(
            r#"<div class="post">
              <div class="post-meta">{owner_link}<span class="post-timestamp">{updated_at}</span></div>
              <form class="edit-project-form" action="https://{DOMAIN}/v1/projects/edit" method="POST">
                <input type="hidden" name="ib_uid" value="{ib_uid}">
                <input type="hidden" name="ib_user" value="{ib_user}">
                <input type="hidden" name="project_id" value="{project_id}">
                <p><strong>Project:</strong></p>
                <input class="post" type="text" name="project" value="{project}" maxlength="255" required>
                <p><strong>Description:</strong></p>
                <input class="post" type="text" name="description" value="{description}" maxlength="1024" required>
                <p><strong>Languages:</strong></p>
                <input class="post" type="text" name="languages" value="{languages}" maxlength="255" required>
                <p><strong>Reinforcements:</strong></p>
                <input class="post" type="text" name="reinforcements" value="{reinforcements}" maxlength="9999">
                <p><strong>Requesting Reinforcements:</strong></p>
                <input type="checkbox" name="reinforcements_request" value="yes" {reinforcements_request_checked}>
                <input class="post-submit" type="submit" value="Save Project">
              </form>
            </div>"#,
            owner_link = owner_link,
            updated_at = escape_html(&row.updated_at),
            ib_uid = ib_uid,
            ib_user = escape_html(ib_user),
            project_id = row.id,
            project = escape_html(&row.project),
            description = escape_html(&row.description),
            languages = escape_html(&row.languages),
            reinforcements = escape_html(row.reinforcements.as_deref().unwrap_or("")),
            reinforcements_request_checked = if row.reinforcements_request == Some(true) { "checked" } else { "" }
          )
        } else {
          let reinforcements_badge = if row.reinforcements_request == Some(true) {
            "<p><em>:[[ :requesting-reinforcements: ]]:</em></p>".to_string()
          } else {
            String::new()
          };
          let reinforcement_usernames: Vec<String> = row
            .reinforcements
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
            .collect();
          let already_reinforcing = session_username
            .as_ref()
            .map(|username| {
              reinforcement_usernames
                .iter()
                .any(|name| name.eq_ignore_ascii_case(username))
            })
            .unwrap_or(false);

          let quick_response_form = if row.reinforcements_request == Some(true)
            && session_uid.is_some()
            && session_uid != Some(row.ib_uid)
            && !already_reinforcing
          {
            format!(
              r#"<form class="quick-response-force-form" action="https://{DOMAIN}/v1/projects/reinforce" method="POST">
                <input type="hidden" name="ib_uid" value="{ib_uid}">
                <input type="hidden" name="ib_user" value="{ib_user}">
                <input type="hidden" name="project_id" value="{project_id}">
                <input type="hidden" name="quick_response_force" value="1">
                <input class="post-submit quick-response-submit" type="submit" value="Respond to Reinforcements Request">
              </form>"#,
              ib_uid = ib_uid,
              ib_user = escape_html(ib_user),
              project_id = row.id
            )
          } else if row.reinforcements_request == Some(true)
            && session_uid.is_some()
            && session_uid != Some(row.ib_uid)
            && already_reinforcing
          {
            format!(
              r#"<form class="quick-response-force-form" action="https://{DOMAIN}/v1/projects/reinforce" method="POST">
                <input type="hidden" name="ib_uid" value="{ib_uid}">
                <input type="hidden" name="ib_user" value="{ib_user}">
                <input type="hidden" name="project_id" value="{project_id}">
                <input type="hidden" name="quick_response_force" value="retreat">
                <input class="post-submit quick-response-submit" type="submit" value="Retreat">
              </form>"#,
              ib_uid = ib_uid,
              ib_user = escape_html(ib_user),
              project_id = row.id
            )
          } else {
            String::new()
          };
          format!(
            r#"<div class="post">
              <div class="post-meta">{owner_link}<span class="post-timestamp">{updated_at}</span></div>
              <p><strong>Project:</strong> {project}</p>
              <p><strong>Description:</strong> {description}</p>
              <p><strong>Languages:</strong> {languages}</p>
              {reinforcements_section}
              {reinforcements_badge}
              {quick_response_form}
            </div>"#,
            owner_link = owner_link,
            updated_at = escape_html(&row.updated_at),
            project = escape_html(&row.project),
            description = render_post_with_hashtags(&row.description, ib_uid, ib_user),
            languages = escape_html(&row.languages),
            reinforcements_section = if let Some(ref r) = row.reinforcements {
              if !r.trim().is_empty() {
                let links: String = r.split(',').map(|name| name.trim()).filter(|name| !name.is_empty()).map(|name| {
                  let total_acks = reinforcement_ack_map
                    .get(&name.to_ascii_lowercase())
                    .copied()
                    .unwrap_or(0);
                  render_project_profile_link(name, total_acks)
                }).collect::<Vec<_>>().join(" ");
                format!("<p><strong>Reinforcements:</strong> {}</p>", links)
              } else {
                String::new()
              }
            } else {
              String::new()
            },
            reinforcements_badge = reinforcements_badge,
            quick_response_form = quick_response_form
          )
        }
      })
      .collect::<Vec<String>>()
      .join("")
  };

  let add_project_form_html = if session_uid == Some(ib_uid) {
    format!(
      r#"<div class="post">
          <form id="add-project-form" action="https://{DOMAIN}/v1/projects" method="POST">
            <input type="hidden" name="ib_uid" value="{ib_uid}">
            <input type="hidden" name="ib_user" value="{ib_user}">
            <p><strong>Project:</strong></p>
            <input class="post" type="text" name="project" maxlength="255" required>
            <p><strong>Description:</strong></p>
            <input class="post" type="text" name="description" maxlength="1024" required>
            <p><strong>Languages:</strong></p>
            <input class="post" type="text" name="languages" maxlength="255" required>
            <input class="post-submit" type="submit" value="Add Project">
          </form>
        </div>"#,
      ib_uid = ib_uid,
      ib_user = escape_html(ib_user)
    )
  } else {
    String::new()
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

  let session_nav_uid = session_uid.unwrap_or(ib_uid);
  let session_nav_user = session_username.as_deref().unwrap_or(ib_user);
  
  let navigation_links = &if session_uid.is_some() {
    format!(
      r#"<nav class="bottom-nav pwa-nav force-horizontal-nav">
    <a class="post-form-display" href="javascript:void(0);">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
          <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
        </svg>
      </div>
    </a>
    <a class="pro-home-display" href="https://{DOMAIN}/v1/profile/{session_ib_user}">
      <div class="nav-icon">
        <img src="https://github.com/{session_ib_user}.png?size=64" alt="Profile"
          style="width: 32px; height: 32px; border-radius: 50%;">
      </div>
    </a>
    <a class="search-display"
      href="https://{DOMAIN}/v1/search-section?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
      </div>
    </a>
    <a class="war-room-display"
      href="https://{DOMAIN}/v1/warroom?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <circle cx="12" cy="12" r="4"></circle>
          <line x1="12" y1="2" x2="12" y2="8"></line>
          <line x1="12" y1="16" x2="12" y2="22"></line>
          <line x1="2" y1="12" x2="8" y2="12"></line>
          <line x1="16" y1="12" x2="22" y2="12"></line>
        </svg>
      </div>
    </a>
    <a class="projects-display"
      href="https://{DOMAIN}/v1/projects?ib_uid={viewed_ib_uid}&amp;ib_user={viewed_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
        </svg>
      </div>
    </a>
    <a class="dm-inbox-display" href="https://{DOMAIN}/v1/inbox?ib_uid={session_ib_uid}&ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
          style="vertical-align: middle; margin-right: 4px;">
          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
          <polyline points="22,6 12,13 2,6"></polyline>
        </svg> <span id="dm-unread-count">{unread_dm_count}</span>
      </div>
    </a>
  </nav>"#,
      session_ib_uid = session_nav_uid,
      session_ib_user = escape_html(session_nav_user),
      viewed_ib_uid = ib_uid,
      viewed_ib_user = escape_html(ib_user),
    )
  } else {
      r#"<div id="actions-section">
      <div class="login-section">
        <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#
        .to_string()
  };

  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &escape_html(ib_user));
  context.insert("domain", &DOMAIN);
  context.insert("advert_html", &advert_html);
  context.insert("add_project_form_html", &add_project_form_html);
  context.insert("projects_html", &projects_html);
  context.insert("navigation_links", &navigation_links);
  
  let html = TEMPLATES.render("projects_mobile.html", &context)
        .map_err(|e| {
            use std::error::Error;
            let mut err_msg = format!("Template error: {}", e);
            let mut cause = e.source();
            while let Some(err) = cause {
                err_msg.push_str(&format!("\nCaused by: {}", err));
                cause = err.source();
            }
            err_msg
        })?;

  Ok(html)
}

pub async fn render_search_projects_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  raw_query: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

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

  let search_terms: Vec<String> = raw_query
    .split_whitespace()
    .map(|term| term.trim().to_lowercase())
    .filter(|term| !term.is_empty())
    .collect();

  let search_results_html = if search_terms.is_empty() {
    "<p><em>:[[ :search-projects: empty-query: ]]:</em></p>".to_string()
  } else {
    let regex_terms: Vec<String> = search_terms
      .iter()
      .map(|term| escape_mysql_regex_token(term))
      .collect();
    let pattern = format!(
      "(^|[[:space:],])({})([[:space:],]|$)",
      regex_terms.join("|")
    );

    let rows = sqlx::query_as::<_, ProjectProfileRow>(
        "SELECT project.id, project.ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(project.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, COALESCE(user.total_acknowledgments, 0) AS total_acknowledgments, project.project, project.description, project.languages, CAST(project.updated_at AS CHAR CHARACTER SET utf8mb4) AS updated_at, COALESCE(project.reinforcements, '') AS reinforcements, COALESCE(project.reinforcements_request, FALSE) AS reinforcements_request FROM project_profile AS project LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(project.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE LOWER(COALESCE(project.languages, '')) REGEXP ? ORDER BY project.updated_at DESC LIMIT 500"
      )
      .bind(&pattern)
      .fetch_all(&state.db_pool)
      .await
      .map_err(|e| format!("Search projects query failed: {}", e))?;

    if rows.is_empty() {
      r#"<div class="notice"><p><em>:[[ :search-projects: is-by: no: is-with: results: ]]:</em></p></div>"#
        .to_string()
    } else {
      let reinforcement_names: HashSet<String> = rows
        .iter()
        .flat_map(|row| {
          row
            .reinforcements
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
            .collect::<Vec<String>>()
        })
        .collect();
      let reinforcement_ack_map = load_project_profile_ack_map(state, &reinforcement_names).await;

      rows
        .iter()
        .map(|row| {
          let owner_link = render_project_profile_link(&row.username, row.total_acknowledgments);
          let reinforcements_badge = if row.reinforcements_request == Some(true) {
            "<p><em>:[[ :requesting-reinforcements: ]]:</em></p>".to_string()
          } else {
            String::new()
          };
          let reinforcement_usernames: Vec<String> = row
            .reinforcements
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
            .collect();
          let already_reinforcing = session_username
            .as_ref()
            .map(|username| {
              reinforcement_usernames
                .iter()
                .any(|name| name.eq_ignore_ascii_case(username))
            })
            .unwrap_or(false);

          let quick_response_form = if row.reinforcements_request == Some(true)
            && session_uid.is_some()
            && session_uid != Some(row.ib_uid)
            && !already_reinforcing
          {
            format!(
              r#"<form class="quick-response-force-form" action="https://{DOMAIN}/v1/projects/reinforce" method="POST">
                <input type="hidden" name="ib_uid" value="{ib_uid}">
                <input type="hidden" name="ib_user" value="{ib_user}">
                <input type="hidden" name="project_id" value="{project_id}">
                <input type="hidden" name="quick_response_force" value="1">
                <input class="post-submit quick-response-submit" type="submit" value="Respond to Reinforcements Request">
              </form>"#,
              ib_uid = ib_uid,
              ib_user = escape_html(ib_user),
              project_id = row.id
            )
          } else if row.reinforcements_request == Some(true)
            && session_uid.is_some()
            && session_uid != Some(row.ib_uid)
            && already_reinforcing
          {
            format!(
              r#"<form class="quick-response-force-form" action="https://{DOMAIN}/v1/projects/reinforce" method="POST">
                <input type="hidden" name="ib_uid" value="{ib_uid}">
                <input type="hidden" name="ib_user" value="{ib_user}">
                <input type="hidden" name="project_id" value="{project_id}">
                <input type="hidden" name="quick_response_force" value="retreat">
                <input class="post-submit quick-response-submit" type="submit" value="Retreat">
              </form>"#,
              ib_uid = ib_uid,
              ib_user = escape_html(ib_user),
              project_id = row.id
            )
          } else {
            String::new()
          };
          format!(
            r#"<div class="post">
              <div class="post-meta">{owner_link}<span class="post-timestamp">{updated_at}</span></div>
              <p><strong>Project:</strong> {project}</p>
              <p><strong>Description:</strong> {description}</p>
              <p><strong>Languages:</strong> {languages}</p>
              {reinforcements_section}
              {reinforcements_badge}
              {quick_response_form}
            </div>"#,
            owner_link = owner_link,
            updated_at = escape_html(&row.updated_at),
            project = escape_html(&row.project),
            description = render_post_with_hashtags(&row.description, ib_uid, ib_user),
            languages = highlight_terms(&row.languages, &search_terms),
            reinforcements_section = if let Some(ref r) = row.reinforcements {
              if !r.trim().is_empty() {
                let links: String = r.split(',').map(|name| name.trim()).filter(|name| !name.is_empty()).map(|name| {
                  let total_acks = reinforcement_ack_map
                    .get(&name.to_ascii_lowercase())
                    .copied()
                    .unwrap_or(0);
                  render_project_profile_link(name, total_acks)
                }).collect::<Vec<_>>().join(" ");
                format!("<p><strong>Reinforcements:</strong> {}</p>", links)
              } else {
                String::new()
              }
            } else {
              String::new()
            },
            reinforcements_badge = reinforcements_badge,
            quick_response_form = quick_response_form
          )
        })
        .collect::<Vec<String>>()
        .join("")
    }
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

  let projects_search_section_html = format!(
    r#"<div id="selected-user-posts-section" class="post-section">
        <div class="notice"><p><em>:[[ :search-project-languages: for-the: {raw_query}: ]]:</em></p></div>
        {search_results_html}
      </div>"#,
    raw_query = escape_html(raw_query),
    search_results_html = search_results_html,
  );

  let ib_pro_result = sqlx::query_as::<_, ProRow>(
      "SELECT ibp, pro, location, services, website, github FROM pro WHERE ib_uid = ?"
    )
    .bind(ib_uid)
    .fetch_optional(&state.db_pool)
    .await
    .map(|opt| opt.unwrap_or_default());

  let source_uid = session_uid.unwrap_or(ib_uid);
  let source_profile_terms = if let Some(uid) = session_uid {
    lookup_profile_terms_by_uid(state, uid)
      .await
      .unwrap_or_else(|| {
        if let Ok(ref pro) = ib_pro_result {
          format!("{} {}", pro.pro, pro.ibp)
        } else {
          String::new()
        }
      })
  } else {
    if let Ok(ref pro) = ib_pro_result {
      format!("{} {}", pro.pro, pro.ibp)
    } else {
      String::new()
    }
  };
  
  let related_userlist_html =
    render_related_userlist_html(state, session_uid, source_uid, &source_profile_terms).await;
  let trending_tags_html = render_trending_tags_html(state, ib_uid, ib_user).await;
  let github_identity_html = render_github_identity_html(state, ib_user).await;
  let sidebar_login_html = if session_uid.is_none() {
    r#"<div id="actions-section">
    <div class="login-section">
      <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
    </div>
  </div>"#
      .to_string()
  } else {
    String::new()
  };
  let follow_section_html = String::new();



  let show_edit_profile_link = session_username
    .as_ref()
    .map(|u| u.eq_ignore_ascii_case(ib_user))
    .unwrap_or(false);

  let edit_profile_link = if show_edit_profile_link {
    format!(r#"<p><a href="https://{DOMAIN}/v1/editprofile?ib_uid={ib_uid}&ib_user={ib_user}">:[[ :edit-profile: ]]:</a></p><br>"#, ib_uid = ib_uid, ib_user = escape_html(ib_user))
  } else {
    String::new()
  };

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

  let viewed_ib_uid = ib_uid;
  let viewed_ib_user = escape_html(ib_user);
  let ib_user_escaped = escape_html(ib_user);
  let ib_ibp_escaped = ib_pro_result.as_ref().map(|p| escape_html(&p.ibp)).unwrap_or_default();
  let ib_pro_escaped = ib_pro_result.as_ref().map(|p| escape_html(&p.pro)).unwrap_or_default();
  let ib_services_escaped = ib_pro_result.as_ref().map(|p| escape_html(&p.services)).unwrap_or_default();
  let ib_location_escaped = ib_pro_result.as_ref().map(|p| escape_html(&p.location)).unwrap_or_default();
  let ib_website_escaped = ib_pro_result.as_ref().map(|p| escape_html(&p.website)).unwrap_or_default();
  let related_users_html = related_userlist_html;
  
  context.insert("ib_user", &ib_user_escaped);
  context.insert("ib_uid", &ib_uid);
  context.insert("viewed_ib_uid", &viewed_ib_uid);
  context.insert("viewed_ib_user", &viewed_ib_user);
  context.insert("domain", &DOMAIN);
  context.insert("navigation_links", &navigation_links);
  context.insert("github_identity_html", &github_identity_html);
  context.insert("sidebar_login_html", &sidebar_login_html);
  context.insert("advert_html", &advert_html);
  context.insert("rank_name", &rank_name);
  context.insert("rank_level", &rank_level);
  context.insert("ib_ibp", &ib_ibp_escaped);
  context.insert("ib_pro", &ib_pro_escaped);
  context.insert("ib_services", &ib_services_escaped);
  context.insert("ib_location", &ib_location_escaped);
  context.insert("ib_website", &ib_website_escaped);
  context.insert("follow_section_html", &follow_section_html);
  context.insert("edit_profile_link", &edit_profile_link);
  context.insert("projects_search_section_html", &projects_search_section_html);
  context.insert("related_users", &related_users_html);
  context.insert("trending_tags", &trending_tags_html);
  context.insert("copyright", &COPYRIGHT);

  let html = TEMPLATES.render("project_search_results.html", &context)
    .map_err(|e| {
        use std::error::Error;
        let mut err_msg = format!("Template error: {}", e);
        let mut cause = e.source();
        while let Some(err) = cause {
            err_msg.push_str(&format!("\nCaused by: {}", err));
            cause = err.source();
        }
        err_msg
    })?;

  Ok(html)
}

pub async fn render_search_projects_mobile_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  raw_query: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

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

  let search_terms: Vec<String> = raw_query
    .split_whitespace()
    .map(|term| term.trim().to_lowercase())
    .filter(|term| !term.is_empty())
    .collect();

  let search_results_html = if search_terms.is_empty() {
    "<p><em>:[[ :search-projects: empty-query: ]]:</em></p>".to_string()
  } else {
    let regex_terms: Vec<String> = search_terms
      .iter()
      .map(|term| escape_mysql_regex_token(term))
      .collect();
    let pattern = format!(
      "(^|[[:space:],])({})([[:space:],]|$)",
      regex_terms.join("|")
    );

    let rows = sqlx::query_as::<_, ProjectProfileRow>(
        "SELECT project.id, project.ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(project.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, COALESCE(user.total_acknowledgments, 0) AS total_acknowledgments, project.project, project.description, project.languages, CAST(project.updated_at AS CHAR CHARACTER SET utf8mb4) AS updated_at, COALESCE(project.reinforcements, '') AS reinforcements, COALESCE(project.reinforcements_request, FALSE) AS reinforcements_request FROM project_profile AS project LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(project.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE LOWER(COALESCE(project.languages, '')) REGEXP ? ORDER BY project.updated_at DESC LIMIT 500"
      )
      .bind(&pattern)
      .fetch_all(&state.db_pool)
      .await
      .map_err(|e| format!("Search projects query failed: {}", e))?;

    if rows.is_empty() {
      "<p><em>:[[ :search-projects: is-by: no: is-with: results: ]]:</em></p>".to_string()
    } else {
      let reinforcement_names: HashSet<String> = rows
        .iter()
        .flat_map(|row| {
          row
            .reinforcements
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
            .collect::<Vec<String>>()
        })
        .collect();
      let reinforcement_ack_map = load_project_profile_ack_map(state, &reinforcement_names).await;

      rows
        .iter()
        .map(|row| {
          let owner_link = render_project_profile_link(&row.username, row.total_acknowledgments);
          let reinforcements_badge = if row.reinforcements_request == Some(true) {
            "<p><em>:[[ :requesting-reinforcements: ]]:</em></p>".to_string()
          } else {
            String::new()
          };
          let reinforcement_usernames: Vec<String> = row
            .reinforcements
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(|item| item.to_string())
            .collect();
          let already_reinforcing = session_username
            .as_ref()
            .map(|username| {
              reinforcement_usernames
                .iter()
                .any(|name| name.eq_ignore_ascii_case(username))
            })
            .unwrap_or(false);

          let quick_response_form = if row.reinforcements_request == Some(true)
            && session_uid.is_some()
            && session_uid != Some(row.ib_uid)
            && !already_reinforcing
          {
            format!(
              r#"<form class="quick-response-force-form" action="https://{DOMAIN}/v1/projects/reinforce" method="POST">
                <input type="hidden" name="ib_uid" value="{ib_uid}">
                <input type="hidden" name="ib_user" value="{ib_user}">
                <input type="hidden" name="project_id" value="{project_id}">
                <input type="hidden" name="quick_response_force" value="1">
                <input class="post-submit quick-response-submit" type="submit" value="Respond to Reinforcements Request">
              </form>"#,
              ib_uid = ib_uid,
              ib_user = escape_html(ib_user),
              project_id = row.id
            )
          } else if row.reinforcements_request == Some(true)
            && session_uid.is_some()
            && session_uid != Some(row.ib_uid)
            && already_reinforcing
          {
            format!(
              r#"<form class="quick-response-force-form" action="https://{DOMAIN}/v1/projects/reinforce" method="POST">
                <input type="hidden" name="ib_uid" value="{ib_uid}">
                <input type="hidden" name="ib_user" value="{ib_user}">
                <input type="hidden" name="project_id" value="{project_id}">
                <input type="hidden" name="quick_response_force" value="retreat">
                <input class="post-submit quick-response-submit" type="submit" value="Retreat">
              </form>"#,
              ib_uid = ib_uid,
              ib_user = escape_html(ib_user),
              project_id = row.id
            )
          } else {
            String::new()
          };
          format!(
            r#"<div class="post">
              <div class="post-meta">{owner_link}<span class="post-timestamp">{updated_at}</span></div>
              <p><strong>Project:</strong> {project}</p>
              <p><strong>Description:</strong> {description}</p>
              <p><strong>Languages:</strong> {languages}</p>
              {reinforcements_section}
              {reinforcements_badge}
              {quick_response_form}
            </div>"#,
            owner_link = owner_link,
            updated_at = escape_html(&row.updated_at),
            project = escape_html(&row.project),
            description = render_post_with_hashtags(&row.description, ib_uid, ib_user),
            languages = highlight_terms(&row.languages, &search_terms),
            reinforcements_section = if let Some(ref r) = row.reinforcements {
              if !r.trim().is_empty() {
                let links: String = r.split(',').map(|name| name.trim()).filter(|name| !name.is_empty()).map(|name| {
                  let total_acks = reinforcement_ack_map
                    .get(&name.to_ascii_lowercase())
                    .copied()
                    .unwrap_or(0);
                  render_project_profile_link(name, total_acks)
                }).collect::<Vec<_>>().join(" ");
                format!("<p><strong>Reinforcements:</strong> {}</p>", links)
              } else {
                String::new()
              }
            } else {
              String::new()
            },
            reinforcements_badge = reinforcements_badge,
            quick_response_form = quick_response_form
          )
        })
        .collect::<Vec<String>>()
        .join("")
    }
  };

  let session_nav_uid = session_uid.unwrap_or(ib_uid);
  let session_nav_user = session_username.as_deref().unwrap_or(ib_user);

  let search_results_section_html = format!(
    r#"<div class="glass-card">
      <div class="notice"><p><em>:[[ :search-project-languages-for: {raw_query}: ]]:</em></p></div>
      {search_results_html}
    </div>"#,
    raw_query = escape_html(raw_query),
    search_results_html = search_results_html,
  );

  let unread_dm_count = if let Some(uid) = session_uid {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM dm WHERE recipient_uid = ? AND read_at IS NULL")
      .bind(uid)
      .fetch_one(&state.db_pool)
      .await
      .unwrap_or(0)
  } else {
    0
  };

  let navigation_links = &if session_uid.is_some() {
    format!(
      r#"<nav class="bottom-nav pwa-nav force-horizontal-nav">
    <a class="post-form-display" href="javascript:void(0);">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
          <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
        </svg>
      </div>
    </a>
    <a class="pro-home-display" href="https://{DOMAIN}/v1/profile/{session_ib_user}">
      <div class="nav-icon">
        <img src="https://github.com/{session_ib_user}.png?size=64" alt="Profile"
          style="width: 32px; height: 32px; border-radius: 50%;">
      </div>
    </a>
    <a class="search-display"
      href="https://{DOMAIN}/v1/search-section?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
      </div>
    </a>
    <a class="war-room-display"
      href="https://{DOMAIN}/v1/warroom?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <circle cx="12" cy="12" r="4"></circle>
          <line x1="12" y1="2" x2="12" y2="8"></line>
          <line x1="12" y1="16" x2="12" y2="22"></line>
          <line x1="2" y1="12" x2="8" y2="12"></line>
          <line x1="16" y1="12" x2="22" y2="12"></line>
        </svg>
      </div>
    </a>
    <a class="projects-display"
      href="https://{DOMAIN}/v1/projects?ib_uid={viewed_ib_uid}&amp;ib_user={viewed_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
        </svg>
      </div>
    </a>
    <a class="dm-inbox-display" href="https://{DOMAIN}/v1/inbox?ib_uid={session_ib_uid}&ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
          style="vertical-align: middle; margin-right: 4px;">
          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
          <polyline points="22,6 12,13 2,6"></polyline>
        </svg> <span id="dm-unread-count">{unread_dm_count}</span>
      </div>
    </a>
  </nav>"#,
      session_ib_uid = session_nav_uid,
      session_ib_user = escape_html(session_nav_user),
      viewed_ib_uid = ib_uid,
      viewed_ib_user = escape_html(ib_user),
    )
  } else {
      r#"<div id="actions-section">
      <div class="login-section">
        <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#
        .to_string()
  };

  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &ib_user);
  context.insert("domain", &DOMAIN);
  context.insert("advert_html", &advert_html);
  context.insert("search_results_section_html", &search_results_section_html);
  context.insert("navigation_links", &navigation_links);
  
  let html = TEMPLATES.render("user_search_results_mobile.html", &context)
    .map_err(|e| {
        use std::error::Error;
        let mut err_msg = format!("Template error: {}", e);
        let mut cause = e.source();
        while let Some(err) = cause {
            err_msg.push_str(&format!("\nCaused by: {}", err));
            cause = err.source();
        }
        err_msg
    })?;

  Ok(html)
}

pub async fn render_user_search_section_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

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

  let session_nav_uid = session_uid.unwrap_or(ib_uid);
  let session_nav_user = session_username.as_deref().unwrap_or(ib_user);

  let unread_dm_count = if let Some(uid) = session_uid {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM dm WHERE recipient_uid = ? AND read_at IS NULL")
      .bind(uid)
      .fetch_one(&state.db_pool)
      .await
      .unwrap_or(0)
  } else {
    0
  };

  let search_section_html = format!(
    r#"<div class="glass-card">
      <div id="user-search-section">
        <form id="user-search-form" action="https://{DOMAIN}/v1/searchusers" method="GET">
          <input type="hidden" name="ib_uid" value="{ib_uid}">
          <input type="hidden" name="ib_user" value="{ib_user}">
          <input type="text" name="query" placeholder="Search Users" required>
          <input type="submit" value="Search Users">
        </form>
        <form id="project-search-form" action="https://{DOMAIN}/v1/searchprojects" method="GET">
          <input type="hidden" name="ib_uid" value="{ib_uid}">
          <input type="hidden" name="ib_user" value="{ib_user}">
          <input type="text" name="query" placeholder="Search Projects" required>
          <input type="submit" value="Search Projects">
        </form>
      </div>
    </div>"#,
    ib_uid = ib_uid,
    ib_user = escape_html(ib_user),
  );

  let navigation_links = &if session_uid.is_some() {
    format!(
      r#"<nav class="bottom-nav pwa-nav force-horizontal-nav">
    <a class="post-form-display" href="javascript:void(0);">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
          <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
        </svg>
      </div>
    </a>
    <a class="pro-home-display" href="https://{DOMAIN}/v1/profile/{session_ib_user}">
      <div class="nav-icon">
        <img src="https://github.com/{session_ib_user}.png?size=64" alt="Profile"
          style="width: 32px; height: 32px; border-radius: 50%;">
      </div>
    </a>
    <a class="search-display"
      href="https://{DOMAIN}/v1/search-section?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
      </div>
    </a>
    <a class="war-room-display"
      href="https://{DOMAIN}/v1/warroom?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <circle cx="12" cy="12" r="4"></circle>
          <line x1="12" y1="2" x2="12" y2="8"></line>
          <line x1="12" y1="16" x2="12" y2="22"></line>
          <line x1="2" y1="12" x2="8" y2="12"></line>
          <line x1="16" y1="12" x2="22" y2="12"></line>
        </svg>
      </div>
    </a>
    <a class="projects-display"
      href="https://{DOMAIN}/v1/projects?ib_uid={viewed_ib_uid}&amp;ib_user={viewed_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
        </svg>
      </div>
    </a>
    <a class="dm-inbox-display" href="https://{DOMAIN}/v1/inbox?ib_uid={session_ib_uid}&ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
          style="vertical-align: middle; margin-right: 4px;">
          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
          <polyline points="22,6 12,13 2,6"></polyline>
        </svg> <span id="dm-unread-count">{unread_dm_count}</span>
      </div>
    </a>
  </nav>"#,
      session_ib_uid = session_nav_uid,
      session_ib_user = escape_html(session_nav_user),
      viewed_ib_uid = ib_uid,
      viewed_ib_user = escape_html(ib_user),
    )
  } else {
      r#"<div id="actions-section">
      <div class="login-section">
        <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#
        .to_string()
  };

  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &escape_html(ib_user));
  context.insert("domain", &DOMAIN);
  context.insert("advert_html", &advert_html);
  context.insert("search_section_html", &search_section_html);
  context.insert("navigation_links", &navigation_links);
  
  let html = TEMPLATES.render("user_search_section_mobile.html", &context)
        .map_err(|e| {
            use std::error::Error;
            let mut err_msg = format!("Template error: {}", e);
            let mut cause = e.source();
            while let Some(err) = cause {
                err_msg.push_str(&format!("\nCaused by: {}", err));
                cause = err.source();
            }
            err_msg
        })?;

  Ok(html)
}

pub async fn render_war_room_posts_chunk(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  session_uid: Option<i64>,
  offset: usize,
  limit: usize,
) -> Result<WarRoomPostsChunk, String> {
  let followers_row = sqlx::query_as::<_, FollowLookupRow>(
      "SELECT username, COALESCE(followers, '') AS followers FROM user WHERE LOWER(username) = LOWER(?) LIMIT 1"
    )
    .bind(ib_user)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| format!("War room followers lookup failed: {}", e))?;

  let follower_usernames: Vec<String> = followers_row
    .as_ref()
    .map(|row| {
      row
        .followers
        .split(',')
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
    })
    .unwrap_or_default();

  let total_followers = follower_usernames.len();
  let start = offset.min(total_followers);
  let end = start.saturating_add(limit).min(total_followers);
  let selected_followers = &follower_usernames[start..end];

  if selected_followers.is_empty() {
    return Ok(WarRoomPostsChunk {
      posts_html: String::new(),
      has_more: false,
      next_offset: end,
      total_followers,
    });
  }

  let mut selected_posts: Vec<(String, PostRow)> = Vec::new();

  for selected_follower in selected_followers {
    let post_row = sqlx::query_as::<_, PostRow>(
        "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks, user.pinned_postid FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE LOWER(COALESCE(CONVERT(user.username USING utf8mb4), '')) = LOWER(?) AND (post.parentid = '' OR post.parentid IS NULL) ORDER BY post.timestamp DESC LIMIT 1"
      )
      .bind(selected_follower)
      .fetch_optional(&state.db_pool)
      .await
      .map_err(|e| format!("War room post lookup failed: {}", e))?;

    if let Some(post_row) = post_row {
      selected_posts.push((selected_follower.clone(), post_row));
    }
  }

  let selected_post_ids: Vec<String> = selected_posts
    .iter()
    .map(|(_, post_row)| post_row.postid.clone())
    .collect();
  let acknowledged_post_ids = acknowledged_post_ids_for_user(&state.db_pool, session_uid, &selected_post_ids).await;

  let mut rendered_posts = String::new();
  for (selected_follower, post_row) in selected_posts {
    rendered_posts += &format!(
      r#"<div class="notice"><p><em>:[[ :war-room: selected-follower: {selected_follower}: ]]:</em></p></div>
      <div class="post" data-postid="{post_id}" data-timestamp="{post_timestamp}">
        {post_meta}
        <div class="post-content">{post_body}</div>
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
      selected_follower = escape_html(&selected_follower),
      post_id = escape_html(&post_row.postid),
      post_timestamp = escape_html(&post_row.timestamp),
      post_meta = render_post_meta(&post_row.ib_uid, &post_row.username, &post_row.timestamp, post_row.user_total_acks),
      post_body = render_post_with_hashtags(&post_row.post, ib_uid, ib_user),
      ack_controls = if session_uid.is_none() || acknowledged_post_ids.contains(&post_row.postid) {
        render_ack_disabled()
      } else {
        render_ack_controls(ib_uid, ib_user, &post_row.postid)
      },
      acknowledged_count = post_row.acknowledged_count,
      post_owner_uid = escape_html(&post_row.ib_uid),
      post_owner_user = escape_html(&post_row.username)
    );
  }

  Ok(WarRoomPostsChunk {
    posts_html: rendered_posts,
    has_more: end < total_followers,
    next_offset: end,
    total_followers,
  })
}

pub async fn render_profile_followers_chunk(
  state: &AppState,
  ib_uid: i64,
  offset: usize,
  limit: usize,
) -> Result<FollowersChunk, String> {
  let viewed_user_row = sqlx::query_as::<_, FollowLookupRow>(
      "SELECT username, COALESCE(followers, '') AS followers FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1"
    )
    .bind(ib_uid.to_string())
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| format!("Viewed user lookup failed: {}", e))?;

  let follower_list: Vec<String> = viewed_user_row
    .as_ref()
    .map(|row| {
      row
        .followers
        .split(',')
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect()
    })
    .unwrap_or_default();

  let total_followers = follower_list.len();
  let start = offset.min(total_followers);
  let end = start.saturating_add(limit).min(total_followers);
  let selected_followers = &follower_list[start..end];

  if selected_followers.is_empty() {
    return Ok(FollowersChunk {
      followers_html: String::new(),
      has_more: false,
      next_offset: end,
      total_followers,
    });
  }

  let follower_username_set: HashSet<String> = selected_followers.iter().cloned().collect();
  let follower_ack_map = load_project_profile_ack_map(state, &follower_username_set).await;

  let followers_html = selected_followers
    .iter()
    .map(|username| {
      let normalized = username.trim().to_ascii_lowercase();
      let total_acks = *follower_ack_map.get(&normalized).unwrap_or(&0);
      let profile_link = render_project_profile_link(username, total_acks);
      format!(
        r#"<p>{profile_link}<button type="button" class="open-dm" data-target-user="{username}">DM</button></p>"#,
        profile_link = profile_link,
        username = escape_html(username)
      )
    })
    .collect::<Vec<String>>()
    .join("");

  Ok(FollowersChunk {
    followers_html,
    has_more: end < total_followers,
    next_offset: end,
    total_followers,
  })
}

pub async fn render_war_room_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

  let war_room_chunk = render_war_room_posts_chunk(state, ib_uid, ib_user, session_uid, 0, 20).await?;

  let war_room_content = if war_room_chunk.total_followers == 0 {
    r#"<div class="notice"><p><em>:[[ :war-room: for-the: followers: is-by: none ]]:</em></p></div>"#.to_string()
  } else if war_room_chunk.posts_html.trim().is_empty() && !war_room_chunk.has_more {
    r#"<div class="notice"><p><em>:[[ :war-room: is-by: no: is-with: follower-posts: ]]:</em></p></div>"#.to_string()
  } else {
    format!(
      r#"<div class="notice"><p><em>:[[ :war-room: for-the: followers-targeted: is-by: {selected_count}: ]]:</em></p></div>{rendered_posts}"#,
      selected_count = war_room_chunk.total_followers,
      rendered_posts = war_room_chunk.posts_html
    )
  };

  let sentinel_html = if war_room_chunk.has_more {
    r#"<div id="posts-load-sentinel"></div>"#
  } else {
    ""
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

  let war_room_html = format!(
    r#"<div id="selected-user-posts-section" class="post-section" data-feed-type="warroom" data-ib-uid="{ib_uid}" data-ib-user="{ib_user}" data-war-room-offset="{war_room_offset}">
        <div class="notice"><p><em>:[[ :war-room: ]]:</em></p></div>
        {war_room_content}
        {sentinel_html}
      </div>"#,
    ib_uid = ib_uid,
    ib_user = escape_html(ib_user),
    war_room_content = war_room_content,
    war_room_offset = war_room_chunk.next_offset,
    sentinel_html = sentinel_html
  );

  let ib_pro = sqlx::query_as::<_, ProRow>(
      "SELECT ibp, pro, location, services, website, github FROM pro WHERE ib_uid = ?"
    )
    .bind(ib_uid)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| format!("Failed to load profile details: {}", e))
    .map(|opt| opt.unwrap_or_default())?;

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

  let source_uid = session_uid.unwrap_or(ib_uid);
  let source_profile_terms = if let Some(uid) = session_uid {
    lookup_profile_terms_by_uid(state, uid)
      .await
      .unwrap_or_else(|| format!("{} {}", ib_pro.pro, ib_pro.ibp))
  } else {
    format!("{} {}", ib_pro.pro, ib_pro.ibp)
  };
  let related_users_html =
    render_related_userlist_html(state, session_uid, source_uid, &source_profile_terms).await;
  let trending_tags_html = render_trending_tags_html(state, ib_uid, ib_user).await;
  let github_identity_html = render_github_identity_html(state, ib_user).await;
  let sidebar_login_html = if session_uid.is_none() {
    r#"<div id="actions-section">
    <div class="login-section">
      <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
    </div>
  </div>"#
      .to_string()
  } else {
    String::new()
  };

  let session_username = if let Some(uid) = session_uid {
    match sqlx::query_as::<_, SessionUserRow>(
      "SELECT username FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1",
    )
    .bind(uid.to_string())
    .fetch_optional(&state.db_pool)
    .await
    {
      Ok(Some(row)) => Some(row.username),
      _ => None,
    }
  } else {
    None
  };

  let show_edit_profile_link = session_username
    .as_ref()
    .map(|u| u.eq_ignore_ascii_case(ib_user))
    .unwrap_or(false);

  let edit_profile_link = if show_edit_profile_link {
    format!(r#"<p><a href="https://{DOMAIN}/v1/editprofile?ib_uid={ib_uid}&ib_user={ib_user}">:[[ :edit-profile: ]]:</a></p><br>"#, ib_uid = ib_uid, ib_user = escape_html(ib_user))
  } else {
    String::new()
  };

  let viewed_ib_uid = ib_uid;
  let viewed_ib_user = escape_html(ib_user);
  let ib_user_escaped = escape_html(ib_user);
  let ib_ibp_escaped = escape_html(&ib_pro.ibp);
  let ib_pro_escaped = escape_html(&ib_pro.pro);
  let ib_services_escaped = escape_html(&ib_pro.services);
  let ib_location_escaped = escape_html(&ib_pro.location);
  let ib_website_escaped = escape_html(&ib_pro.website);
  
  context.insert("ib_user", &ib_user_escaped);
  context.insert("ib_uid", &ib_uid);
  context.insert("viewed_ib_uid", &viewed_ib_uid);
  context.insert("viewed_ib_user", &viewed_ib_user);
  context.insert("domain", &DOMAIN);
  context.insert("navigation_links", &navigation_links);
  context.insert("github_identity_html", &github_identity_html);
  context.insert("sidebar_login_html", &sidebar_login_html);
  context.insert("advert_html", &advert_html);
  context.insert("war_room_html", &war_room_html);
  context.insert("rank_name", &rank_name);
  context.insert("rank_level", &rank_level);
  context.insert("ib_ibp", &ib_ibp_escaped);
  context.insert("ib_pro", &ib_pro_escaped);
  context.insert("ib_services", &ib_services_escaped);
  context.insert("ib_location", &ib_location_escaped);
  context.insert("ib_website", &ib_website_escaped);
  context.insert("edit_profile_link", &edit_profile_link);
  context.insert("related_users", &related_users_html);
  context.insert("trending_tags", &trending_tags_html);
  context.insert("copyright", &COPYRIGHT);
  
  let html = TEMPLATES.render("war_room.html", &context)
    .map_err(|e| {
        use std::error::Error;
        let mut err_msg = format!("Template error: {}", e);
        let mut cause = e.source();
        while let Some(err) = cause {
            err_msg.push_str(&format!("\nCaused by: {}", err));
            cause = err.source();
        }
        err_msg
    })?;

  Ok(html)
}

pub async fn render_war_room_mobile_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

  let war_room_chunk = render_war_room_posts_chunk(state, ib_uid, ib_user, session_uid, 0, 20).await?;

  let war_room_content = if war_room_chunk.total_followers == 0 {
    "<div class=\"notice\"><p><em>:[[ :war-room: for-the: followers: is-by: none: ]]:</em></p></div>".to_string()
  } else if war_room_chunk.posts_html.trim().is_empty() && !war_room_chunk.has_more {
    "<div class=\"notice\"><p><em>:[[ :war-room: is-by: no: is-with: follower-posts: ]]:</em></p></div>".to_string()
  } else {
    format!(
      r#"<div class="notice"><p><em>:[[ :war-room: for-the: followers-targeted: is-by: {selected_count}: ]]:</em></p></div>{rendered_posts}"#,
      selected_count = war_room_chunk.total_followers,
      rendered_posts = war_room_chunk.posts_html
    )
  };

  let sentinel_html = if war_room_chunk.has_more {
    r#"<div id="posts-load-sentinel"></div>"#
  } else {
    ""
  };

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

  let session_nav_uid = session_uid.unwrap_or(ib_uid);
  let session_nav_user = session_username.as_deref().unwrap_or(ib_user);

  let unread_dm_count = if let Some(uid) = session_uid {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM dm WHERE recipient_uid = ? AND read_at IS NULL")
      .bind(uid)
      .fetch_one(&state.db_pool)
      .await
      .unwrap_or(0)
  } else {
    0
  };

  let navigation_links = &if session_uid.is_some() {
    format!(
      r#"<nav class="bottom-nav pwa-nav force-horizontal-nav">
    <a class="post-form-display" href="javascript:void(0);">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
          <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
        </svg>
      </div>
    </a>
    <a class="pro-home-display" href="https://{DOMAIN}/v1/profile/{session_ib_user}">
      <div class="nav-icon">
        <img src="https://github.com/{session_ib_user}.png?size=64" alt="Profile"
          style="width: 32px; height: 32px; border-radius: 50%;">
      </div>
    </a>
    <a class="search-display"
      href="https://{DOMAIN}/v1/search-section?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
      </div>
    </a>
    <a class="war-room-display"
      href="https://{DOMAIN}/v1/warroom?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <circle cx="12" cy="12" r="4"></circle>
          <line x1="12" y1="2" x2="12" y2="8"></line>
          <line x1="12" y1="16" x2="12" y2="22"></line>
          <line x1="2" y1="12" x2="8" y2="12"></line>
          <line x1="16" y1="12" x2="22" y2="12"></line>
        </svg>
      </div>
    </a>
    <a class="projects-display"
      href="https://{DOMAIN}/v1/projects?ib_uid={viewed_ib_uid}&amp;ib_user={viewed_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
        </svg>
      </div>
    </a>
    <a class="dm-inbox-display" href="https://{DOMAIN}/v1/inbox?ib_uid={session_ib_uid}&ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
          style="vertical-align: middle; margin-right: 4px;">
          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
          <polyline points="22,6 12,13 2,6"></polyline>
        </svg> <span id="dm-unread-count">{unread_dm_count}</span>
      </div>
    </a>
  </nav>"#,
      session_ib_uid = session_nav_uid,
      session_ib_user = escape_html(session_nav_user),
      viewed_ib_uid = ib_uid,
      viewed_ib_user = escape_html(ib_user),
    )
  } else {
      r#"<div id="actions-section">
      <div class="login-section">
        <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#
        .to_string()
  };

  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &escape_html(ib_user));
  context.insert("domain", &DOMAIN);
  context.insert("advert_html", &advert_html);
  context.insert("war_room_content", &war_room_content);
  context.insert("sentinel_html", &sentinel_html);
  context.insert("navigation_links", &navigation_links);

  let html = TEMPLATES.render("war_room_mobile.html", &context)
        .map_err(|e| {
            use std::error::Error;
            let mut err_msg = format!("Template error: {}", e);
            let mut cause = e.source();
            while let Some(err) = cause {
                err_msg.push_str(&format!("\nCaused by: {}", err));
                cause = err.source();
            }
            err_msg
        })?;

  Ok(html)
}

pub async fn render_inbox_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  session_uid: Option<i64>,
  requested_target_user: Option<&str>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

  let inbox_users = load_inbox_contacts(state, ib_uid, ib_user).await?;

  let initial_contacts: Vec<String> = inbox_users.iter().take(20).cloned().collect();
  let contacts_next_offset = initial_contacts.len();
  let contacts_has_more = inbox_users.len() > initial_contacts.len();
  let contacts_sentinel_html = if contacts_has_more {
    r#"<div id="dm-contacts-load-sentinel"></div>"#
  } else {
    ""
  };

  let default_target_user = requested_target_user
    .map(|value| value.trim())
    .filter(|value| !value.is_empty())
    .map(|value| value.to_string())
    .or_else(|| inbox_users.first().cloned())
    .unwrap_or_default();

  let contact_list_html = render_inbox_contacts_html(&initial_contacts);

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

  let dm_inbox_html = format!(
    r#"<div id="selected-user-posts-section" class="post-section">
        <div class="notice"><p><em>:[[ :direct-message-inbox: ]]:</em></p></div>
        <div id="dm-inbox-layout">
          <div id="dm-contact-list" data-ib-uid="{ib_uid}" data-ib-user="{ib_user}" data-contacts-offset="{contacts_next_offset}">{contact_list_html}{contacts_sentinel_html}</div>
          <div id="dm-panel" style="display:block;">
            <p><strong>:[[ :direct-messages: ]]: <span id="dm-target-user">{default_target_user}</span></strong></p>
            <div id="dm-message-status"></div>
            <div id="dm-thread"></div>
            <form id="dm-form" action="https://{DOMAIN}/v1/dm/send" method="POST">
              <input type="hidden" id="dm-target-user-input" name="target_user" value="{default_target_user}">
              <input type="text" id="dm-message-input" name="message" maxlength="1024" placeholder="Type a direct message" required>
              <input type="submit" value="Send">
            </form>
          </div>
        </div>
      </div>"#,
    ib_uid = ib_uid,
    ib_user = escape_html(ib_user),
    contacts_next_offset = contacts_next_offset,
    contact_list_html = contact_list_html,
    contacts_sentinel_html = contacts_sentinel_html,
    default_target_user = escape_html(&default_target_user)
  );

  let ib_pro_result = sqlx::query_as::<_, ProRow>(
      "SELECT ibp, pro, location, services, website, github FROM pro WHERE ib_uid = ?"
    )
    .bind(ib_uid)
    .fetch_optional(&state.db_pool)
    .await
    .map(|opt| opt.unwrap_or_default());

  let source_uid = session_uid.unwrap_or(ib_uid);
  let source_profile_terms = if let Some(uid) = session_uid {
    lookup_profile_terms_by_uid(state, uid)
      .await
      .unwrap_or_else(|| {
        if let Ok(ref pro) = ib_pro_result {
          format!("{} {}", pro.pro, pro.ibp)
        } else {
          String::new()
        }
      })
  } else {
    if let Ok(ref pro) = ib_pro_result {
      format!("{} {}", pro.pro, pro.ibp)
    } else {
      String::new()
    }
  };
  
  let related_userlist_html =
    render_related_userlist_html(state, session_uid, source_uid, &source_profile_terms).await;
  let trending_tags_html = render_trending_tags_html(state, ib_uid, ib_user).await;
  let github_identity_html = render_github_identity_html(state, ib_user).await;
  let sidebar_login_html = if session_uid.is_none() {
    r#"<div id="actions-section">
    <div class="login-section">
      <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
    </div>
  </div>"#
      .to_string()
  } else {
    String::new()
  };

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

  let follow_section_html = String::new();

  let show_edit_profile_link = session_username
    .as_ref()
    .map(|username| username.eq_ignore_ascii_case(ib_user))
    .unwrap_or(false);

  let edit_profile_link = if show_edit_profile_link {
    format!(r#"<p><a href="https://{DOMAIN}/v1/editprofile?ib_uid={ib_uid}&ib_user={ib_user}">:[[ :edit-profile: ]]:</a></p><br>"#, ib_uid = ib_uid, ib_user = escape_html(ib_user))
  } else {
    String::new()
  };

  let viewed_ib_uid = ib_uid;
  let viewed_ib_user = escape_html(ib_user);
  let ib_user_escaped = escape_html(ib_user);
  let ib_ibp_escaped = ib_pro_result.as_ref().map(|p| escape_html(&p.ibp)).unwrap_or_default();
  let ib_pro_escaped = ib_pro_result.as_ref().map(|p| escape_html(&p.pro)).unwrap_or_default();
  let ib_services_escaped = ib_pro_result.as_ref().map(|p| escape_html(&p.services)).unwrap_or_default();
  let ib_location_escaped = ib_pro_result.as_ref().map(|p| escape_html(&p.location)).unwrap_or_default();
  let ib_website_escaped = ib_pro_result.as_ref().map(|p| escape_html(&p.website)).unwrap_or_default();
  let related_users_html = related_userlist_html;
  
  context.insert("ib_user", &ib_user_escaped);
  context.insert("ib_uid", &ib_uid);
  context.insert("viewed_ib_uid", &viewed_ib_uid);
  context.insert("viewed_ib_user", &viewed_ib_user);
  context.insert("domain", &DOMAIN);
  context.insert("navigation_links", &navigation_links);
  context.insert("github_identity_html", &github_identity_html);
  context.insert("sidebar_login_html", &sidebar_login_html);
  context.insert("advert_html", &advert_html);
  context.insert("rank_name", &rank_name);
  context.insert("rank_level", &rank_level);
  context.insert("ib_ibp", &ib_ibp_escaped);
  context.insert("ib_pro", &ib_pro_escaped);
  context.insert("ib_services", &ib_services_escaped);
  context.insert("ib_location", &ib_location_escaped);
  context.insert("ib_website", &ib_website_escaped);
  context.insert("follow_section_html", &follow_section_html);
  context.insert("edit_profile_link", &edit_profile_link);
  context.insert("dm_inbox_html", &dm_inbox_html);
  context.insert("related_users", &related_users_html);
  context.insert("trending_tags", &trending_tags_html);
  context.insert("copyright", &COPYRIGHT);
  
  let html = TEMPLATES.render("dm_inbox.html", &context)
    .map_err(|e| {
        use std::error::Error;
        let mut err_msg = format!("Template error: {}", e);
        let mut cause = e.source();
        while let Some(err) = cause {
            err_msg.push_str(&format!("\nCaused by: {}", err));
            cause = err.source();
        }
        err_msg
    })?;

  Ok(html)
}

pub async fn render_inbox_mobile_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  session_uid: Option<i64>,
  requested_target_user: Option<&str>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

  let inbox_users = load_inbox_contacts(state, ib_uid, ib_user).await?;

  let initial_contacts: Vec<String> = inbox_users.iter().take(20).cloned().collect();
  let contacts_next_offset = initial_contacts.len();
  let contacts_has_more = inbox_users.len() > initial_contacts.len();
  let contacts_sentinel_html = if contacts_has_more {
    r#"<div id="dm-contacts-load-sentinel"></div>"#
  } else {
    ""
  };

  let default_target_user = requested_target_user
    .map(|value| value.trim())
    .filter(|value| !value.is_empty())
    .map(|value| value.to_string())
    .or_else(|| inbox_users.first().cloned())
    .unwrap_or_default();

  let contact_list_html = render_inbox_contacts_html(&initial_contacts);

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

  let session_nav_uid = session_uid.unwrap_or(ib_uid);
  let session_nav_user = session_username.as_deref().unwrap_or(ib_user);

  let dm_inbox_html = format!(
    r#"<div class="glass-card">
      <div class="notice"><p><em>:[[ :direct-message-inbox: ]]:</em></p></div>
      <div id="dm-inbox-layout">
        <div id="dm-contact-list" data-ib-uid="{ib_uid}" data-ib-user="{ib_user}" data-contacts-offset="{contacts_next_offset}">{contact_list_html}{contacts_sentinel_html}</div>
        <div id="dm-panel" style="display:block;">
          <p><strong>:[[ :direct-messages: ]]: <span id="dm-target-user">{default_target_user}</span></strong></p>
          <div id="dm-message-status"></div>
          <div id="dm-thread"></div>
          <form id="dm-form" action="https://{DOMAIN}/v1/dm/send" method="POST">
            <input type="hidden" id="dm-target-user-input" name="target_user" value="{default_target_user}">
            <input type="text" id="dm-message-input" name="message" maxlength="1024" placeholder="Type a direct message" required>
            <input type="submit" value="Send">
          </form>
        </div>
      </div>
    </div>"#,
    ib_uid = ib_uid,
    ib_user = escape_html(ib_user),
    contacts_next_offset = contacts_next_offset,
    contact_list_html = contact_list_html,
    contacts_sentinel_html = contacts_sentinel_html,
    default_target_user = escape_html(&default_target_user),
  );

  let unread_dm_count = if let Some(uid) = session_uid {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM dm WHERE recipient_uid = ? AND read_at IS NULL")
      .bind(uid)
      .fetch_one(&state.db_pool)
      .await
      .unwrap_or(0)
  } else {
    0
  };

  let navigation_links = &if session_uid.is_some() {
    format!(
      r#"<nav class="bottom-nav pwa-nav force-horizontal-nav">
    <a class="post-form-display" href="javascript:void(0);">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
          <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
        </svg>
      </div>
    </a>
    <a class="pro-home-display" href="https://{DOMAIN}/v1/profile/{session_ib_user}">
      <div class="nav-icon">
        <img src="https://github.com/{session_ib_user}.png?size=64" alt="Profile"
          style="width: 32px; height: 32px; border-radius: 50%;">
      </div>
    </a>
    <a class="search-display"
      href="https://{DOMAIN}/v1/search-section?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
      </div>
    </a>
    <a class="war-room-display"
      href="https://{DOMAIN}/v1/warroom?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <circle cx="12" cy="12" r="4"></circle>
          <line x1="12" y1="2" x2="12" y2="8"></line>
          <line x1="12" y1="16" x2="12" y2="22"></line>
          <line x1="2" y1="12" x2="8" y2="12"></line>
          <line x1="16" y1="12" x2="22" y2="12"></line>
        </svg>
      </div>
    </a>
    <a class="projects-display"
      href="https://{DOMAIN}/v1/projects?ib_uid={viewed_ib_uid}&amp;ib_user={viewed_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
        </svg>
      </div>
    </a>
    <a class="dm-inbox-display" href="https://{DOMAIN}/v1/inbox?ib_uid={session_ib_uid}&ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
          style="vertical-align: middle; margin-right: 4px;">
          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
          <polyline points="22,6 12,13 2,6"></polyline>
        </svg> <span id="dm-unread-count">{unread_dm_count}</span>
      </div>
    </a>
  </nav>"#,
      session_ib_uid = session_nav_uid,
      session_ib_user = escape_html(session_nav_user),
      viewed_ib_uid = ib_uid,
      viewed_ib_user = escape_html(ib_user),
    )
  } else {
      r#"<div id="actions-section">
      <div class="login-section">
        <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#
        .to_string()
  };

  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &escape_html(ib_user));
  context.insert("domain", &DOMAIN);
  context.insert("advert_html", &advert_html);
  context.insert("dm_inbox_html", &dm_inbox_html);
  context.insert("navigation_links", &navigation_links);
  
  let html = TEMPLATES.render("dm_inbox_mobile.html", &context)
        .map_err(|e| {
            use std::error::Error;
            let mut err_msg = format!("Template error: {}", e);
            let mut cause = e.source();
            while let Some(err) = cause {
                err_msg.push_str(&format!("\nCaused by: {}", err));
                cause = err.source();
            }
            err_msg
        })?;

  Ok(html)
}

pub async fn render_single_post_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  pid: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

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

  let post = sqlx::query_as::<_, PostRow>(
      "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks, user.pinned_postid FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE post.ib_uid = ? AND post.postid = ? LIMIT 1"
    )
    .bind(ib_uid)
    .bind(pid)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| format!("Post lookup failed: {}", e))?;

  let post = match post {
    Some(post) => post,
    None => return Err(format!("Post not found: {}", pid)),
  };

  let replies = sqlx::query_as::<_, PostRow>(
      "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks, user.pinned_postid FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE post.parentid = ? ORDER BY post.timestamp ASC"
    )
    .bind(pid)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| format!("Reply lookup failed: {}", e))?;

  let mut visible_post_ids = vec![post.postid.clone()];
  visible_post_ids.extend(replies.iter().map(|reply| reply.postid.clone()));
  let acknowledged_post_ids = acknowledged_post_ids_for_user(&state.db_pool, session_uid, &visible_post_ids).await;

  let mut replies_html = String::new();

  if !replies.is_empty() {
    for reply in replies {
      let reply_owner_uid = escape_html(&reply.ib_uid);
      let can_manage_reply = session_uid.is_some() && session_uid == reply.ib_uid.parse::<i64>().ok();
      let reply_manage_actions = if can_manage_reply {
        format!(
          r#"<form class="delete-post-form" action="https://{DOMAIN}/v1/deletepost" method="POST">
              <input type="hidden" name="ib_uid" value="{page_ib_uid}">
              <input type="hidden" name="ib_user" value="{page_ib_user}">
              <input type="hidden" name="pid" value="{reply_post_id}">
              <input type="hidden" name="root_pid" value="{root_pid}">
              <input type="hidden" name="post_owner_uid" value="{reply_owner_uid}">
            </form>
            <form class="edit-post-form" action="https://{DOMAIN}/v1/editpost" method="GET">
              <input type="hidden" name="ib_uid" value="{page_ib_uid}">
              <input type="hidden" name="ib_user" value="{page_ib_user}">
              <input type="hidden" name="pid" value="{reply_post_id}">
              <input type="hidden" name="root_pid" value="{root_pid}">
              <input type="hidden" name="post_owner_uid" value="{reply_owner_uid}">
            </form>
            <a href="javascript:void(0);" class="edit-post">:[[ :edit: ]]:</a>&nbsp;&nbsp;&nbsp;&nbsp;<a href="javascript:void(0);" class="delete-post">:[[ :delete: ]]:</a>&nbsp;&nbsp;&nbsp;&nbsp;"#,
          page_ib_uid = ib_uid,
          page_ib_user = escape_html(ib_user),
          root_pid = escape_html(pid),
          reply_post_id = escape_html(&reply.postid),
          reply_owner_uid = reply_owner_uid,
        )
      } else {
        String::new()
      };

      replies_html += &format!(
        r#"
        <div class="post reply-post" data-postid="{reply_post_id}" data-timestamp="{reply_post_timestamp}">
          {reply_post_meta}
          <div class="post-content">{reply_post_body}</div>
          <div class="post-actions">
            {reply_ack_controls}
            {reply_manage_actions}
          </div>
          <p class="acknowledged-count">Acknowleged {reply_acknowledged_count} times.</p>
        </div>"#,
        reply_post_id = escape_html(&reply.postid),
        reply_post_timestamp = escape_html(&reply.timestamp),
        reply_post_meta = render_post_meta(&reply.ib_uid, &reply.username, &reply.timestamp, reply.user_total_acks),
        reply_post_body = render_post_with_hashtags(&reply.post, ib_uid, ib_user),
        reply_ack_controls = if session_uid.is_none() || acknowledged_post_ids.contains(&reply.postid) {
          render_ack_disabled()
        } else {
          render_ack_controls(ib_uid, ib_user, &reply.postid)
        },
        reply_acknowledged_count = reply.acknowledged_count,
        reply_manage_actions = reply_manage_actions
      );
    }
  }

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

  let single_post_html = format!(
    r#"<div id="selected-user-posts-section" class="post-section">
        <div class="post" data-postid="{ib_post_id}" data-timestamp="{ib_post_timestamp}">
          {post_meta}
          <div class="post-content">{post_body}</div>
          <div class="post-actions">
            {ack_controls}
            <p><a href="javascript:void(0);" class="copy-link">:[[ :copy-link: ]]:</a>&nbsp;&nbsp;&nbsp;&nbsp;<a href="javascript:void(0);" class="pin-post-link">:[[ :pin-post: ]]:</a></p>
          </div>
          <p class="acknowledged-count">Acknowleged {ib_post_acknowledged_count} times.</p>
        </div>
        {replies_html}
        <div id="post-form-section" style="display:block;">
          <form id="reply-form" action="https://{DOMAIN}/v1/reply" method="POST">
            <input type="hidden" name="ib_uid" value="{ib_uid}">
            <input type="hidden" name="ib_user" value="{ib_user}">
            <input type="hidden" name="pid" value="{ib_post_id}">
            <input type="text" class="post" name="post" maxlength="1024" required>
            <br>
            <input class="post-submit" type="submit" value="Reply">
          </form>
        </div>
      </div>"#,
    ib_uid = ib_uid,
    ib_user = escape_html(ib_user),
    ib_post_id = escape_html(&post.postid),
    ib_post_timestamp = escape_html(&post.timestamp),
    ib_post_acknowledged_count = post.acknowledged_count,
    post_meta = render_post_meta(&post.ib_uid, &post.username, &post.timestamp, post.user_total_acks),
    ack_controls = if session_uid.is_none() || acknowledged_post_ids.contains(&post.postid) {
      render_ack_disabled()
    } else {
      render_ack_controls(ib_uid, ib_user, &post.postid)
    },
    post_body = render_post_with_hashtags(&post.post, ib_uid, ib_user),
    replies_html = replies_html
  );

  let ib_pro = sqlx::query_as::<_, ProRow>(
      "SELECT ibp, pro, location, services, website, github FROM pro WHERE ib_uid = ?"
    )
    .bind(ib_uid)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| format!("Failed to load profile details: {}", e))
    .map(|opt| opt.unwrap_or_default())?;

  let source_uid = session_uid.unwrap_or(ib_uid);
  let source_profile_terms = if let Some(uid) = session_uid {
    lookup_profile_terms_by_uid(state, uid)
      .await
      .unwrap_or_else(|| format!("{} {}", ib_pro.pro, ib_pro.ibp))
  } else {
    format!("{} {}", ib_pro.pro, ib_pro.ibp)
  };
  let related_users_html =
    render_related_userlist_html(state, session_uid, source_uid, &source_profile_terms).await;
  let trending_tags_html = render_trending_tags_html(state, ib_uid, ib_user).await;

  let viewed_ib_uid = ib_uid;
  let viewed_ib_user = escape_html(ib_user);
  let ib_user_escaped = escape_html(ib_user);
  let ib_ibp_escaped = escape_html(&ib_pro.ibp);
  let ib_pro_escaped = escape_html(&ib_pro.pro);
  let ib_services_escaped = escape_html(&ib_pro.services);
  let ib_location_escaped = escape_html(&ib_pro.location);
  let ib_website_escaped = escape_html(&ib_pro.website);

  let viewed_user_row = sqlx::query_as::<_, FollowLookupRow>(
      "SELECT username, COALESCE(followers, '') AS followers FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1"
    )
    .bind(ib_uid.to_string())
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| format!("Viewed user lookup failed: {}", e))?;

  let viewed_username = viewed_user_row
    .as_ref()
    .map(|row| row.username.clone())
    .unwrap_or_else(|| ib_user.to_string());

  let show_edit_profile_link = session_username
    .as_ref()
    .map(|username| username.eq_ignore_ascii_case(&viewed_username))
    .unwrap_or(false);
  
    let edit_profile_link = if show_edit_profile_link {
    format!(r#"<p><a href="https://{DOMAIN}/v1/editprofile?ib_uid={ib_uid}&ib_user={ib_user}">:[[ :edit-profile: ]]:</a></p><br>"#, ib_uid = ib_uid, ib_user = escape_html(ib_user))
  } else {
    String::new()
  };

  let github_identity_html = render_github_identity_html(state, ib_user).await;
  let sidebar_login_html = if session_uid.is_none() {
    r#"<div id="actions-section">
    <div class="login-section">
      <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
    </div>
  </div>"#
      .to_string()
  } else {
    String::new()
  };

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

  context.insert("ib_user", &ib_user_escaped);
  context.insert("ib_uid", &ib_uid);
  context.insert("viewed_ib_uid", &viewed_ib_uid);
  context.insert("viewed_ib_user", &viewed_ib_user);
  context.insert("domain", &DOMAIN);
  context.insert("navigation_links", &navigation_links);
  context.insert("github_identity_html", &github_identity_html);
  context.insert("sidebar_login_html", &sidebar_login_html);
  context.insert("advert_html", &advert_html);
  context.insert("post", &post.post);
  context.insert("single_post_html", &single_post_html);
  context.insert("rank_name", &rank_name);
  context.insert("rank_level", &rank_level);
  context.insert("ib_ibp", &ib_ibp_escaped);
  context.insert("ib_pro", &ib_pro_escaped);
  context.insert("ib_services", &ib_services_escaped);
  context.insert("ib_location", &ib_location_escaped);
  context.insert("ib_website", &ib_website_escaped);
  context.insert("edit_profile_link", &edit_profile_link);
  context.insert("related_users", &related_users_html);
  context.insert("trending_tags", &trending_tags_html);
  context.insert("copyright", &COPYRIGHT);
  context.insert("meta_tags", &extract_meta_tags_from_post(&post.post));
  
  let html = TEMPLATES.render("single_post.html", &context)
    .map_err(|e| {
        use std::error::Error;
        let mut err_msg = format!("Template error: {}", e);
        let mut cause = e.source();
        while let Some(err) = cause {
            err_msg.push_str(&format!("\nCaused by: {}", err));
            cause = err.source();
        }
        err_msg
    })?;

  Ok(html)
}

pub async fn render_single_post_mobile_html(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  pid: &str,
  session_uid: Option<i64>,
) -> Result<String, String> {
  let mut context = Context::new();
  let advert_html = render_advert_html(state).await;

  let post = sqlx::query_as::<_, PostRow>(
      "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks, user.pinned_postid FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE post.ib_uid = ? AND post.postid = ? LIMIT 1"
    )
    .bind(ib_uid)
    .bind(pid)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| format!("Post lookup failed: {}", e))?;

  let post = match post {
    Some(post) => post,
    None => return Err(format!("Post not found: {}", pid)),
  };

  let replies = sqlx::query_as::<_, PostRow>(
      "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks, user.pinned_postid FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE post.parentid = ? ORDER BY post.timestamp ASC"
    )
    .bind(pid)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| format!("Reply lookup failed: {}", e))?;

  let mut visible_post_ids = vec![post.postid.clone()];
  visible_post_ids.extend(replies.iter().map(|reply| reply.postid.clone()));
  let acknowledged_post_ids = acknowledged_post_ids_for_user(&state.db_pool, session_uid, &visible_post_ids).await;

  let mut replies_html = String::new();

  if !replies.is_empty() {
    for reply in replies {
      let reply_owner_uid = escape_html(&reply.ib_uid);
      let can_manage_reply = session_uid.is_some() && session_uid == reply.ib_uid.parse::<i64>().ok();
      let reply_manage_actions = if can_manage_reply {
        format!(
          r#"<form class="delete-post-form" action="https://{DOMAIN}/v1/deletepost" method="POST">
              <input type="hidden" name="ib_uid" value="{page_ib_uid}">
              <input type="hidden" name="ib_user" value="{page_ib_user}">
              <input type="hidden" name="pid" value="{reply_post_id}">
              <input type="hidden" name="root_pid" value="{root_pid}">
              <input type="hidden" name="post_owner_uid" value="{reply_owner_uid}">
            </form>
            <form class="edit-post-form" action="https://{DOMAIN}/v1/editpost" method="GET">
              <input type="hidden" name="ib_uid" value="{page_ib_uid}">
              <input type="hidden" name="ib_user" value="{page_ib_user}">
              <input type="hidden" name="pid" value="{reply_post_id}">
              <input type="hidden" name="root_pid" value="{root_pid}">
              <input type="hidden" name="post_owner_uid" value="{reply_owner_uid}">
            </form>
            <a href="javascript:void(0);" class="edit-post">:[[ :edit: ]]:</a>&nbsp;&nbsp;&nbsp;&nbsp;<a href="javascript:void(0);" class="delete-post">:[[ :delete: ]]:</a>&nbsp;&nbsp;&nbsp;&nbsp;"#,
          page_ib_uid = ib_uid,
          page_ib_user = escape_html(ib_user),
          root_pid = escape_html(pid),
          reply_post_id = escape_html(&reply.postid),
          reply_owner_uid = reply_owner_uid,
        )
      } else {
        String::new()
      };

      replies_html += &format!(
        r#"
        <div class="post reply-post" data-postid="{reply_post_id}" data-timestamp="{reply_post_timestamp}">
          {reply_post_meta}
          <div class="post-content">{reply_post_body}</div>
          <div class="post-actions">
            {reply_ack_controls}
            {reply_manage_actions}
          </div>
          <p class="acknowledged-count">Acknowleged {reply_acknowledged_count} times.</p>
        </div>"#,
        reply_post_id = escape_html(&reply.postid),
        reply_post_timestamp = escape_html(&reply.timestamp),
        reply_post_meta = render_post_meta(&reply.ib_uid, &reply.username, &reply.timestamp, reply.user_total_acks),
        reply_post_body = render_post_with_hashtags(&reply.post, ib_uid, ib_user),
        reply_ack_controls = if session_uid.is_none() || acknowledged_post_ids.contains(&reply.postid) {
          render_ack_disabled()
        } else {
          render_ack_controls(ib_uid, ib_user, &reply.postid)
        },
        reply_acknowledged_count = reply.acknowledged_count,
        reply_manage_actions = reply_manage_actions
      );
    }
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

  let session_nav_uid = session_uid.unwrap_or(ib_uid);
  let session_nav_user = session_username.as_deref().unwrap_or(ib_user);

  let single_post_html = format!(
    r#"<div class="glass-card">
      <div id="selected-user-posts-section">
        <div class="post" data-postid="{ib_post_id}" data-timestamp="{ib_post_timestamp}">
          {post_meta}
          <div class="post-content">{post_body}</div>
          <div class="post-actions">
            {ack_controls}
            <p><a href="javascript:void(0);" class="copy-link">:[[ :copy-link: ]]:</a>&nbsp;&nbsp;&nbsp;&nbsp;<a href="javascript:void(0);" class="pin-post-link">:[[ :pin-post: ]]:</a></p>
          </div>
          <p class="acknowledged-count">Acknowleged {ib_post_acknowledged_count} times.</p>
        </div>
        {replies_html}
        <div id="post-form-section" style="display:block;">
          <form id="reply-form" action="https://{DOMAIN}/v1/reply" method="POST">
            <input type="hidden" name="ib_uid" value="{ib_uid}">
            <input type="hidden" name="ib_user" value="{ib_user}">
            <input type="hidden" name="pid" value="{ib_post_id}">
            <input type="text" class="post" name="post" maxlength="1024" required>
            <br>
            <input class="post-submit" type="submit" value="Reply">
          </form>
        </div>
      </div>
    </div>"#,
    ib_user = escape_html(ib_user),
    ib_uid = ib_uid,
    ib_post_id = escape_html(&post.postid),
    ib_post_timestamp = escape_html(&post.timestamp),
    ib_post_acknowledged_count = post.acknowledged_count,
    post_meta = render_post_meta(&post.ib_uid, &post.username, &post.timestamp, post.user_total_acks),
    ack_controls = if session_uid.is_none() || acknowledged_post_ids.contains(&post.postid) {
      render_ack_disabled()
    } else {
      render_ack_controls(ib_uid, ib_user, &post.postid)
    },
    post_body = render_post_with_hashtags(&post.post, ib_uid, ib_user),
    replies_html = replies_html
  );

  let unread_dm_count = if let Some(uid) = session_uid {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM dm WHERE recipient_uid = ? AND read_at IS NULL")
      .bind(uid)
      .fetch_one(&state.db_pool)
      .await
      .unwrap_or(0)
  } else {
    0
  };

  let navigation_links = &if session_uid.is_some() {
    format!(
      r#"<nav class="bottom-nav pwa-nav force-horizontal-nav">
    <a class="post-form-display" href="javascript:void(0);">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
          <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
        </svg>
      </div>
    </a>
    <a class="pro-home-display" href="https://{DOMAIN}/v1/profile/{session_ib_user}">
      <div class="nav-icon">
        <img src="https://github.com/{session_ib_user}.png?size=64" alt="Profile"
          style="width: 32px; height: 32px; border-radius: 50%;">
      </div>
    </a>
    <a class="search-display"
      href="https://{DOMAIN}/v1/search-section?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
      </div>
    </a>
    <a class="war-room-display"
      href="https://{DOMAIN}/v1/warroom?ib_uid={session_ib_uid}&amp;ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <circle cx="12" cy="12" r="4"></circle>
          <line x1="12" y1="2" x2="12" y2="8"></line>
          <line x1="12" y1="16" x2="12" y2="22"></line>
          <line x1="2" y1="12" x2="8" y2="12"></line>
          <line x1="16" y1="12" x2="22" y2="12"></line>
        </svg>
      </div>
    </a>
    <a class="projects-display"
      href="https://{DOMAIN}/v1/projects?ib_uid={viewed_ib_uid}&amp;ib_user={viewed_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
        </svg>
      </div>
    </a>
    <a class="dm-inbox-display" href="https://{DOMAIN}/v1/inbox?ib_uid={session_ib_uid}&ib_user={session_ib_user}">
      <div class="nav-icon">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
          stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
          style="vertical-align: middle; margin-right: 4px;">
          <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path>
          <polyline points="22,6 12,13 2,6"></polyline>
        </svg> <span id="dm-unread-count">{unread_dm_count}</span>
      </div>
    </a>
  </nav>"#,
      session_ib_uid = session_nav_uid,
      session_ib_user = escape_html(session_nav_user),
      viewed_ib_uid = ib_uid,
      viewed_ib_user = escape_html(ib_user),
    )
  } else {
      r#"<div id="actions-section">
      <div class="login-section">
        <p style="width: 100%; text-align: center; margin: 0;"><a href="/v1/auth/github">Login with GitHub</a></p>
      </div>
    </div>"#
        .to_string()
  };

  context.insert("ib_uid", &ib_uid);
  context.insert("ib_user", &escape_html(ib_user));
  context.insert("domain", &DOMAIN);
  context.insert("advert_html", &advert_html);
  context.insert("post", &post.post);
  context.insert("single_post_html", &single_post_html);
  context.insert("navigation_links", &navigation_links);
  context.insert("meta_tags", &extract_meta_tags_from_post(&post.post));
  
  let html = TEMPLATES.render("single_post_mobile.html", &context)
        .map_err(|e| {
            use std::error::Error;
            let mut err_msg = format!("Template error: {}", e);
            let mut cause = e.source();
            while let Some(err) = cause {
                err_msg.push_str(&format!("\nCaused by: {}", err));
                cause = err.source();
            }
            err_msg
        })?;

  Ok(html)
}

pub async fn render_embed_post_response(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  pid: &str,
) -> HttpResponse {
  let post = match sqlx::query_as::<_, PostRow>(
      "SELECT CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) AS ib_uid, CAST(COALESCE(CONVERT(user.username USING utf8mb4), CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4)) AS CHAR CHARACTER SET utf8mb4) AS username, post.postid, post.post, post.timestamp, COALESCE(post.acknowledged_count, 0) AS acknowledged_count, COALESCE(user.total_acknowledgments, 0) AS user_total_acks, user.pinned_postid FROM post AS post LEFT JOIN user AS user ON CONVERT(user.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(post.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE post.ib_uid = ? AND post.postid = ? LIMIT 1"
    )
    .bind(ib_uid)
    .bind(pid)
    .fetch_optional(&state.db_pool)
    .await {
        Ok(Some(p)) => p,
        _ => return HttpResponse::NotFound().body("Post not found"),
    };

  let single_post_html = format!(
    r#"<div class="glass-card" style="margin: 0; padding: 10px;">
        <div class="post" data-postid="{ib_post_id}" data-timestamp="{ib_post_timestamp}" style="border: none;">
          {post_meta}
          <div class="post-content">{post_body}</div>
        </div>
      </div>"#,
    ib_post_id = escape_html(&post.postid),
    ib_post_timestamp = escape_html(&post.timestamp),
    post_meta = render_post_meta(&post.ib_uid, &post.username, &post.timestamp, post.user_total_acks),
    post_body = render_post_with_hashtags(&post.post, ib_uid, ib_user),
  );

  let mut context = Context::new();
  context.insert("single_post_html", &single_post_html);
  context.insert("iframe_id", &format!("embed_{}", pid));

  let html = TEMPLATES.render("embed_post.html", &context).unwrap_or_else(|e| format!("Error: {}", e));
  HttpResponse::Ok().content_type("text/html; charset=utf-8").body(html)
}

pub async fn render_show_post_response(
  req: &HttpRequest,
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
  pid: &str,
  session_uid: Option<i64>,
) -> HttpResponse {
  let html_result = if is_mobile_device(req) {
    render_single_post_mobile_html(state, ib_uid, ib_user, pid, session_uid).await
  } else {
    render_single_post_html(state, ib_uid, ib_user, pid, session_uid).await
  };

  match html_result {
    Ok(html) => HttpResponse::Ok()
      .content_type("text/html; charset=utf-8")
      .body(html),
    Err(err) if err.starts_with("Post not found:") => HttpResponse::NotFound()
      .content_type("text/html; charset=utf-8")
      .body(format!(
        "<!DOCTYPE html><html lang=\"en-US\"><head><meta charset=\"UTF-8\"><title>Post Not Found</title></head><body><p>{}</p></body></html>",
        escape_html(&err)
      )),
    Err(err) => HttpResponse::InternalServerError().body(err),
  }
}

fn extract_youtube_video_id(url: &str) -> Option<String> {
  if let Some(pos) = url.find("youtube.com/watch?v=") {
    let start = pos + "youtube.com/watch?v=".len();
    let rest = &url[start..];
    let end = rest.find('&').unwrap_or(rest.len());
    Some(rest[..end].to_string())
  } else if let Some(pos) = url.find("youtu.be/") {
    let start = pos + "youtu.be/".len();
    let rest = &url[start..];
    let end = rest.find('?').unwrap_or(rest.len());
    Some(rest[..end].to_string())
  } else {
    None
  }
}

fn extract_imgur_info(url: &str) -> Option<String> {
  if let Some(pos) = url.find("i.imgur.com/") {
    let rest = &url[pos + "i.imgur.com/".len()..];
    let end = rest.find('?').unwrap_or(rest.len());
    let path = &rest[..end];
    let path_lower = path.to_lowercase();
    
    if path_lower.ends_with(".mp4") {
        return Some(format!(r#"<div class="imgur-preview-wrapper" style="display:flex; justify-content:center; width:100%; margin: 10px 0;"><video src="https://i.imgur.com/{}" controls loop muted playsinline style="max-width: 100%; max-height: 500px; border-radius: 8px;"></video></div>"#, escape_html(path)));
    } else if path_lower.ends_with(".gifv") {
        let mp4_path = format!("{}.mp4", &path[..path.len() - 5]);
        return Some(format!(r#"<div class="imgur-preview-wrapper" style="display:flex; justify-content:center; width:100%; margin: 10px 0;"><video src="https://i.imgur.com/{}" controls loop muted playsinline style="max-width: 100%; max-height: 500px; border-radius: 8px;"></video></div>"#, escape_html(&mp4_path)));
    } else {
        return Some(format!(r#"<div class="imgur-preview-wrapper" style="display:flex; justify-content:center; width:100%; margin: 10px 0;"><a href="https://i.imgur.com/{0}" target="_blank" rel="noopener"><img src="https://i.imgur.com/{0}" style="max-width: 100%; max-height: 500px; border-radius: 8px;" alt="Imgur Preview"></a></div>"#, escape_html(path)));
    }
  } else if let Some(pos) = url.find("imgur.com/") {
    let rest = &url[pos + "imgur.com/".len()..];
    let end = rest.find('?').unwrap_or(rest.len());
    let path = &rest[..end];
    let path = path.trim_end_matches('/');
    
    if path.is_empty() {
        return None;
    }

    if !path.starts_with("a/") && !path.starts_with("gallery/") {
        let path_lower = path.to_lowercase();
        if path_lower.ends_with(".mp4") {
            return Some(format!(r#"<div class="imgur-preview-wrapper" style="display:flex; justify-content:center; width:100%; margin: 10px 0;"><video src="https://i.imgur.com/{}" controls loop muted playsinline style="max-width: 100%; max-height: 500px; border-radius: 8px;"></video></div>"#, escape_html(path)));
        } else if path_lower.ends_with(".gifv") {
            let mp4_path = format!("{}.mp4", &path[..path.len() - 5]);
            return Some(format!(r#"<div class="imgur-preview-wrapper" style="display:flex; justify-content:center; width:100%; margin: 10px 0;"><video src="https://i.imgur.com/{}" controls loop muted playsinline style="max-width: 100%; max-height: 500px; border-radius: 8px;"></video></div>"#, escape_html(&mp4_path)));
        } else {
            return Some(format!(r#"<div class="imgur-preview-wrapper" style="display:flex; justify-content:center; width:100%; margin: 10px 0;"><a href="https://i.imgur.com/{0}.jpg" target="_blank" rel="noopener"><img src="https://i.imgur.com/{0}.jpg" style="max-width: 100%; max-height: 500px; border-radius: 8px;" alt="Imgur Preview"></a></div>"#, escape_html(path)));
        }
    } else {
        return Some(format!(r#"<div class="imgur-preview-wrapper" style="display:flex; justify-content:center; width:100%; margin: 10px 0;"><iframe scrolling="no" src="https://imgur.com/{}/embed?pub=true" style="width: 100%; max-width: 560px; height: 500px; border: none; border-radius: 8px;" allowfullscreen="true"></iframe></div>"#, escape_html(path)));
    }
  }
  None
}

fn extract_rumble_info(url: &str) -> Option<String> {
    if let Some(pos) = url.find("rumble.com/embed/") {
        let start = pos + "rumble.com/embed/".len();
        let rest = &url[start..];
        let end = rest.find('/').unwrap_or(rest.len());
        let end = rest[..end].find('?').unwrap_or(end);
        let id = &rest[..end];
        if !id.is_empty() {
            return Some(format!(r#"<div class="youtube-preview-wrapper" style="display:flex; justify-content:center; width:100%; margin: 10px 0;"><div class="youtube-preview-container" style="width:100%; max-width:560px; margin: 0 auto; display: block; position: relative; overflow: hidden; padding-bottom: 56.25%; height: 0; border-radius: 8px;"><iframe src="https://rumble.com/embed/{}/" title="Rumble video player" allow="autoplay; encrypted-media; picture-in-picture" allowfullscreen width="100%" height="100%" style="border:0; position:absolute; top:0; left:0; width:100%; height:100%;"></iframe></div></div>"#, escape_html(id)));
        }
    }
    None
}

fn extract_is_by_info(url: &str) -> Option<String> {
    if let Some(pos) = url.find("is-by.pro/v1/showpost?") {
        let start = pos + "is-by.pro/v1/showpost?".len();
        let query_string = &url[start..];
        let id = format!("embed_{}", uuid::Uuid::new_v4().simple());
        
        return Some(format!(
            r#"<div class="isby-preview-wrapper" style="width:100%; margin: 10px 0;">
                <iframe id="{}" src="https://is-by.pro/v1/embedpost?{}" style="width: 100%; border: 1px solid #3F3F3F; border-radius: 8px; min-height: 250px; max-height: 500px; overflow-y: auto;" scrolling="yes"></iframe>
            </div>"#,
            id, escape_html(query_string)
        ));
    }
    None
}

pub fn extract_meta_tags_from_post(raw_text: &str) -> String {
  let chars: Vec<(usize, char)> = raw_text.char_indices().collect();
  let mut index = 0usize;

  while index < chars.len() {
    let (byte_pos, _ch) = chars[index];
    let previous_is_token_char = index > 0
      && (chars[index - 1].1.is_ascii_alphanumeric() || chars[index - 1].1 == '_');
    let scheme_url_start = raw_text[byte_pos..].starts_with("http://") || raw_text[byte_pos..].starts_with("https://");
    let www_url_start = !previous_is_token_char && raw_text[byte_pos..].starts_with("www.");

    if scheme_url_start || www_url_start {
      let mut url_end_index = index;

      while url_end_index < chars.len() && !chars[url_end_index].1.is_whitespace() {
        url_end_index += 1;
      }

      let url_end_byte = if url_end_index < chars.len() {
        chars[url_end_index].0
      } else {
        raw_text.len()
      };

      let raw_url = &raw_text[byte_pos..url_end_byte];
      let (trimmed_url, _) = trim_trailing_url_punctuation(raw_url);
      let href = if www_url_start {
        format!("https://{}", trimmed_url)
      } else {
        trimmed_url.to_string()
      };

      if let Some(video_id) = extract_youtube_video_id(&href) {
        return format!(
          r#"<meta property="og:type" content="video.other">
<meta property="og:video:url" content="https://www.youtube.com/embed/{video_id}">
<meta property="og:video:secure_url" content="https://www.youtube.com/embed/{video_id}">
<meta property="og:video:type" content="text/html">
<meta property="og:video:width" content="1280">
<meta property="og:video:height" content="720">
<meta name="twitter:card" content="player">
<meta name="twitter:player" content="https://www.youtube.com/embed/{video_id}">
<meta name="twitter:player:width" content="1280">
<meta name="twitter:player:height" content="720">"#,
          video_id = escape_html(&video_id)
        );
      } else if let Some(pos) = href.find("i.imgur.com/") {
          let rest = &href[pos + "i.imgur.com/".len()..];
          let end = rest.find('?').unwrap_or(rest.len());
          let path = &rest[..end];
          let path_lower = path.to_lowercase();
          
          if path_lower.ends_with(".mp4") || path_lower.ends_with(".gifv") {
              let mp4_path = if path_lower.ends_with(".gifv") {
                  format!("{}.mp4", &path[..path.len() - 5])
              } else {
                  path.to_string()
              };
              return format!(
                r#"<meta property="og:type" content="video.other">
<meta property="og:video:url" content="https://i.imgur.com/{mp4_path}">
<meta property="og:video:secure_url" content="https://i.imgur.com/{mp4_path}">
<meta property="og:video:type" content="video/mp4">
<meta name="twitter:card" content="player">
<meta name="twitter:player:stream" content="https://i.imgur.com/{mp4_path}">
<meta name="twitter:player:stream:content_type" content="video/mp4">"#,
                mp4_path = escape_html(&mp4_path)
              );
          } else {
              return format!(
                r#"<meta property="og:type" content="image">
<meta property="og:image" content="https://i.imgur.com/{img_path}">
<meta property="og:image:secure_url" content="https://i.imgur.com/{img_path}">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:image" content="https://i.imgur.com/{img_path}">"#,
                img_path = escape_html(path)
              );
          }
      } else if let Some(pos) = href.find("imgur.com/") {
          let rest = &href[pos + "imgur.com/".len()..];
          let end = rest.find('?').unwrap_or(rest.len());
          let path = &rest[..end];
          let path = path.trim_end_matches('/');
          if !path.is_empty() {
              if !path.starts_with("a/") && !path.starts_with("gallery/") {
                  let path_lower = path.to_lowercase();
                  if path_lower.ends_with(".mp4") || path_lower.ends_with(".gifv") {
                      let mp4_path = if path_lower.ends_with(".gifv") {
                          format!("{}.mp4", &path[..path.len() - 5])
                      } else {
                          path.to_string()
                      };
                      return format!(
                        r#"<meta property="og:type" content="video.other">
<meta property="og:video:url" content="https://i.imgur.com/{mp4_path}">
<meta property="og:video:secure_url" content="https://i.imgur.com/{mp4_path}">
<meta property="og:video:type" content="video/mp4">
<meta name="twitter:card" content="player">
<meta name="twitter:player:stream" content="https://i.imgur.com/{mp4_path}">
<meta name="twitter:player:stream:content_type" content="video/mp4">"#,
                        mp4_path = escape_html(&mp4_path)
                      );
                  } else {
                      return format!(
                        r#"<meta property="og:type" content="image">
<meta property="og:image" content="https://i.imgur.com/{img_path}.jpg">
<meta property="og:image:secure_url" content="https://i.imgur.com/{img_path}.jpg">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:image" content="https://i.imgur.com/{img_path}.jpg">"#,
                        img_path = escape_html(path)
                      );
                  }
              }
          }
      } else if let Some(pos) = href.find("rumble.com/embed/") {
          let start = pos + "rumble.com/embed/".len();
          let rest = &href[start..];
          let end = rest.find('/').unwrap_or(rest.len());
          let end = rest[..end].find('?').unwrap_or(end);
          let id = &rest[..end];
          if !id.is_empty() {
              return format!(
                r#"<meta property="og:type" content="video.other">
<meta property="og:video:url" content="https://rumble.com/embed/{rumble_id}/">
<meta property="og:video:secure_url" content="https://rumble.com/embed/{rumble_id}/">
<meta property="og:video:type" content="text/html">
<meta property="og:video:width" content="1280">
<meta property="og:video:height" content="720">
<meta name="twitter:card" content="player">
<meta name="twitter:player" content="https://rumble.com/embed/{rumble_id}/">
<meta name="twitter:player:width" content="1280">
<meta name="twitter:player:height" content="720">"#,
                rumble_id = escape_html(id)
              );
          }
      }
    }
    index += 1;
  }
  String::new()
}

pub fn render_post_with_hashtags(raw_text: &str, ib_uid: i64, ib_user: &str) -> String {
  let mut rendered = String::new();
  let chars: Vec<(usize, char)> = raw_text.char_indices().collect();
  let mut cursor = 0usize;
  let mut index = 0usize;

  while index < chars.len() {
    let (byte_pos, ch) = chars[index];
    let previous_is_token_char = index > 0
      && (chars[index - 1].1.is_ascii_alphanumeric() || chars[index - 1].1 == '_');
    let scheme_url_start = raw_text[byte_pos..].starts_with("http://") || raw_text[byte_pos..].starts_with("https://");
    let www_url_start = !previous_is_token_char && raw_text[byte_pos..].starts_with("www.");

    if scheme_url_start || www_url_start {
      let mut url_end_index = index;

      while url_end_index < chars.len() && !chars[url_end_index].1.is_whitespace() {
        url_end_index += 1;
      }

      rendered.push_str(&escape_html(&raw_text[cursor..byte_pos]));

      let url_end_byte = if url_end_index < chars.len() {
        chars[url_end_index].0
      } else {
        raw_text.len()
      };

      let raw_url = &raw_text[byte_pos..url_end_byte];
      let (trimmed_url, trailing_punctuation) = trim_trailing_url_punctuation(raw_url);
      let href = if www_url_start {
        format!("https://{}", trimmed_url)
      } else {
        trimmed_url.to_string()
      };

      if let Some(video_id) = extract_youtube_video_id(&href) {
        rendered.push_str(&format!(
          r#"<div class="youtube-preview-wrapper" style="display:flex; justify-content:center; width:100%; margin: 10px 0;"><div class="youtube-preview-container" style="width:100%; max-width:560px; margin: 0 auto; display: block; position: relative; overflow: hidden; padding-bottom: 56.25%; height: 0; border-radius: 8px;"><iframe src="https://www.youtube.com/embed/{video_id}" title="YouTube video player" allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share" allowfullscreen width="100%" height="100%" style="border:0; position:absolute; top:0; left:0; width:100%; height:100%;"></iframe></div></div>"#,
          video_id = escape_html(&video_id)
        ));
      } else if let Some(imgur_html) = extract_imgur_info(&href) {
        rendered.push_str(&imgur_html);
      } else if let Some(rumble_html) = extract_rumble_info(&href) {
        rendered.push_str(&rumble_html);
      } else if let Some(is_by_html) = extract_is_by_info(&href) {
        rendered.push_str(&is_by_html);
      } else {
        rendered.push_str(&format!(
          r#"<a class="post-link" href="{href}" target="_blank" rel="noopener">{label}</a>"#,
          href = escape_html(&href),
          label = escape_html(trimmed_url)
        ));
      }
      rendered.push_str(&escape_html(trailing_punctuation));

      cursor = url_end_byte;
      index = url_end_index;
      continue;
    }

    if (ch == '#' || ch == '@') && !previous_is_token_char {
      let mut token_end_index = index + 1;

      while token_end_index < chars.len() {
        let next_char = chars[token_end_index].1;
        if next_char.is_ascii_alphanumeric() || next_char == '_' {
          token_end_index += 1;
        } else {
          break;
        }
      }

      if token_end_index > index + 1 {
        rendered.push_str(&escape_html(&raw_text[cursor..byte_pos]));

        let token_start_byte = byte_pos + ch.len_utf8();
        let token_end_byte = if token_end_index < chars.len() {
          chars[token_end_index].0
        } else {
          raw_text.len()
        };

        let token_value = &raw_text[token_start_byte..token_end_byte];
        let href = if ch == '#' {
          format!(
            "https://{DOMAIN}/v1/searchposts?ib_uid={ib_uid}&ib_user={ib_user}&tag=%23{tag}",
            ib_uid = ib_uid,
            ib_user = url_encode_component(ib_user),
            tag = url_encode_component(token_value)
          )
        } else {
          format!(
            "https://{DOMAIN}/v1/profile/{username}",
            username = url_encode_component(token_value)
          )
        };

        let class_name = if ch == '#' { "post-tag" } else { "post-mention" };
        rendered.push_str(&format!(
          r#"<a class="{class_name}" href="{href}">{prefix}{token}</a>"#,
          class_name = class_name,
          href = href,
          prefix = ch,
          token = escape_html(token_value)
        ));

        cursor = token_end_byte;
        index = token_end_index;
        continue;
      }
    }

    index += 1;
  }

  if cursor < raw_text.len() {
    rendered.push_str(&escape_html(&raw_text[cursor..]));
  }

  rendered
}

pub fn render_post_meta(ib_uid: &str, username: &str, timestamp: &str, user_total_acks: i64) -> String {
  let profile_target = if username.trim().is_empty() {
    ib_uid
  } else {
    username
  };

  let rank_info = get_rank_info(user_total_acks);
  let rank_icon = rank_info.asset;
  let glow_style = if rank_info.level >= 11 {
    "filter: drop-shadow(0 0 3px #fff) drop-shadow(0 0 5px #fff); "
  } else {
    ""
  };

  format!(
    r#"<div class="post-meta"><a class="post-author" href="https://{DOMAIN}/v1/profile/{profile_target}"><img class="post-author-avatar" src="https://github.com/{profile_target}.png?size=32" alt="{username}" width="32" height="32"><img class="rank-insignia" src="/images/ranks/{rank_icon}" alt="Rank" width="16" height="16" style="{glow_style}vertical-align: middle; margin-left: 4px; margin-right: 4px;">{username}</a><span class="post-timestamp">{timestamp}</span></div>"#,
    profile_target = escape_html(profile_target),
    username = escape_html(username),
    timestamp = escape_html(timestamp),
    rank_icon = rank_icon,
    glow_style = glow_style
  )
}

pub fn render_project_profile_link(username: &str, user_total_acks: i64) -> String {
  let rank_info = get_rank_info(user_total_acks);
  let glow_style = if rank_info.level >= 11 {
    "filter: drop-shadow(0 0 3px #fff) drop-shadow(0 0 5px #fff); "
  } else {
    ""
  };

  format!(
    r#"<a class="post-author" href="https://{DOMAIN}/v1/profile/{owner_username}"><img class="post-author-avatar" src="https://github.com/{owner_username}.png?size=32" alt="{owner_username}" width="32" height="32" style="margin-right:6px;vertical-align:middle;"><img class="rank-insignia" src="/images/ranks/{rank_icon}" alt="Rank" width="16" height="16" style="{glow_style}vertical-align: middle; margin-left: 4px; margin-right: 4px;">{owner_username}</a>"#,
    owner_username = escape_html(username),
    rank_icon = rank_info.asset,
    glow_style = glow_style,
  )
}

pub fn render_ack_controls(page_ib_uid: i64, page_ib_user: &str, post_id: &str) -> String {
  if page_ib_uid <= 0 {
    return String::from(r#"<span class="ack-post-disabled">:[[ACK]]:</span>"#);
  }

  format!(
    r#"<form class="ack-post-form" action="https://{DOMAIN}/v1/ackpost" method="POST">
      <input type="hidden" name="ib_uid" value="{ib_uid}">
      <input type="hidden" name="ib_user" value="{ib_user}">
      <input type="hidden" name="pid" value="{post_id}">
    </form>
    <a href="javascript:void(0);" class="ack-post">:[[ACK]]:</a>"#,
    ib_uid = page_ib_uid,
    ib_user = escape_html(page_ib_user),
    post_id = escape_html(post_id)
  )
}

pub fn render_ack_disabled() -> String {
  String::from(r#"<span class="ack-post-disabled">:[[ACK]]:</span>"#)
}

pub fn render_inbox_contacts_html(inbox_users: &[String]) -> String {
  if inbox_users.is_empty() {
    return "<p><em>:[[ :is-by: none: for-the: direct-message-contacts: ]]:</em></p>".to_string();
  }

  inbox_users
    .iter()
    .map(|username| {
      format!(
        r#"<p><button type="button" class="open-dm" data-target-user="{username}">{username}</button></p>"#,
        username = escape_html(username)
      )
    })
    .collect::<Vec<String>>()
    .join("")
}

pub async fn related_users(state: &AppState, session_uid: Option<i64>) -> String {
  let empty_result = "<p><em>:[[ :is-by: none: for-the: related-users: ]]:</em></p>".to_string();

  let uid = match session_uid {
    Some(id) => id,
    None => return empty_result,
  };

  let user_pro_row = sqlx::query_scalar::<_, String>("SELECT pro FROM pro WHERE ib_uid = ?")
    .bind(uid)
    .fetch_optional(&state.db_pool)
    .await
    .map(|opt| opt.unwrap_or_default());

  let pro_value = match user_pro_row {
    Ok(p) if !p.trim().is_empty() => p,
    _ => return empty_result,
  };

  let session_username = sqlx::query_scalar::<_, String>(
      "SELECT username FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1"
    )
    .bind(uid.to_string())
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None)
    .unwrap_or_default();

  let related = if session_username.is_empty() {
    sqlx::query_as::<_, RelatedUsernameRankRow>(
      "SELECT u.username, COALESCE(u.total_acknowledgments, 0) AS total_acknowledgments FROM pro p JOIN user u ON CONVERT(u.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(p.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE p.pro = ? AND p.ib_uid != ? ORDER BY u.total_acknowledgments DESC LIMIT 50",
    )
    .bind(&pro_value)
    .bind(uid)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default()
  } else {
    sqlx::query_as::<_, RelatedUsernameRankRow>(
      "SELECT u.username, COALESCE(u.total_acknowledgments, 0) AS total_acknowledgments FROM pro p JOIN user u ON CONVERT(u.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(p.ib_uid AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE p.pro = ? AND p.ib_uid != ? AND FIND_IN_SET(LOWER(?), LOWER(REPLACE(COALESCE(u.followers, ''), ' ', ''))) = 0 ORDER BY u.total_acknowledgments DESC LIMIT 50",
    )
    .bind(&pro_value)
    .bind(uid)
    .bind(&session_username)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default()
  };

  if related.is_empty() {
    return empty_result;
  }

  let mut html = String::new();
  for row in related {
    let rank_info = get_rank_info(row.total_acknowledgments);
    let glow_style = if rank_info.level >= 11 {
      "filter: drop-shadow(0 0 3px #fff) drop-shadow(0 0 5px #fff); "
    } else {
      ""
    };
    let safe_user = escape_html(&row.username);
    let encoded_user = url_encode_component(&row.username);

    let link = format!(
      r#"<p><a class="post-author" href="/v1/profile/{encoded_user}"><img class="post-author-avatar" src="https://github.com/{encoded_user}.png?size=32" alt="{safe_user}" width="32" height="32" style="margin-right:6px;vertical-align:middle;"><img class="rank-insignia" src="/images/ranks/{rank_icon}" alt="Rank" width="16" height="16" style="{glow_style}vertical-align: middle; margin-left: 4px; margin-right: 4px;">{safe_user}</a></p>"#,
      encoded_user = encoded_user,
      safe_user = safe_user,
      rank_icon = rank_info.asset,
      glow_style = glow_style,
    );
    html.push_str(&link);
  }

  html
}

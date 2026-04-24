use sqlx::{mysql::MySqlPoolOptions, MySqlPool};
use std::collections::{HashSet, HashMap};
use crate::{extract_hashtags, models::*, utils::*};
use crate::{MYSQL_DATABASE, MYSQL_USER};

pub async fn acknowledged_post_ids_for_user(pool: &MySqlPool, viewer_uid: Option<i64>, post_ids: &[String]) -> HashSet<String> {
  let Some(uid) = viewer_uid else {
    return HashSet::new();
  };

  if post_ids.is_empty() {
    return HashSet::new();
  }

  let placeholders = vec!["?"; post_ids.len()].join(", ");
  let sql = format!(
    "SELECT postid FROM post_ack WHERE ib_uid = ? AND postid IN ({})",
    placeholders
  );

  let mut query = sqlx::query_scalar::<_, String>(&sql).bind(uid);
  for post_id in post_ids {
    query = query.bind(post_id);
  }

  match query.fetch_all(pool).await {
    Ok(rows) => rows.into_iter().collect(),
    Err(_) => HashSet::new(),
  }
}
pub async fn create_db_pool() -> Result<MySqlPool, sqlx::Error> {
  let mysql_host = std::env::var("MYSQL_HOST")
    .unwrap_or_else(|_| String::from("localhost"));
  let mysql_port = std::env::var("MYSQL_PORT")
    .ok()
    .and_then(|value| value.parse::<u16>().ok())
    .unwrap_or(3306);
  let mysql_password = std::env::var("MYSQL_PASSWORD")
    .expect("Missing MYSQL_PASSWORD in environment file or shell");

  let server_options = sqlx::mysql::MySqlConnectOptions::new()
    .host(&mysql_host)
    .port(mysql_port)
    .username(MYSQL_USER)
    .password(&mysql_password);

  // Bootstrap the database itself before connecting with a selected schema.
  let bootstrap_pool = MySqlPoolOptions::new()
    .max_connections(1)
    .connect_with(server_options.clone())
    .await?;

  let safe_database_name = MYSQL_DATABASE.replace('`', "");
  let create_database_sql = format!(
    "CREATE DATABASE IF NOT EXISTS `{}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
    safe_database_name
  );

  sqlx::query(&create_database_sql)
    .execute(&bootstrap_pool)
    .await?;

  drop(bootstrap_pool);

  MySqlPoolOptions::new()
    .max_connections(5)
    .connect_with(server_options.database(MYSQL_DATABASE))
    .await
}
pub async fn ensure_database_schema(pool: &MySqlPool) -> Result<(), sqlx::Error> {
  sqlx::query(
    "CREATE TABLE IF NOT EXISTS user (ib_uid VARCHAR(64) PRIMARY KEY, username VARCHAR(255) NOT NULL, followers TEXT NOT NULL, total_acknowledgments BIGINT NOT NULL DEFAULT 0)",
  )
  .execute(pool)
  .await?;

  let total_acknowledgments_exists = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'user' AND COLUMN_NAME = 'total_acknowledgments'"
    )
    .fetch_one(pool)
    .await?;

  if total_acknowledgments_exists == 0 {
    sqlx::query("ALTER TABLE user ADD COLUMN total_acknowledgments BIGINT NOT NULL DEFAULT 0")
      .execute(pool)
      .await?;
  }

  // Add Full-Text Index to user table if it doesn't exist
  let _ = sqlx::query("ALTER TABLE user ADD FULLTEXT INDEX ft_username (username)")
    .execute(pool)
    .await;

  sqlx::query(
    "CREATE TABLE IF NOT EXISTS pro (ib_uid BIGINT PRIMARY KEY, github VARCHAR(255) NOT NULL, ibp TEXT NOT NULL, pro TEXT NOT NULL, services TEXT NOT NULL, location TEXT NOT NULL, website TEXT NOT NULL)",
  )
  .execute(pool)
  .await?;

  // Add Full-Text Index to pro table
  let _ = sqlx::query("ALTER TABLE pro ADD FULLTEXT INDEX ft_pro_search (ibp, pro, services, location, website, github)")
    .execute(pool)
    .await;

  sqlx::query(
    "CREATE TABLE IF NOT EXISTS post (ib_uid BIGINT NOT NULL, postid VARCHAR(64) PRIMARY KEY, parentid VARCHAR(64) NOT NULL DEFAULT '', post VARCHAR(1024) NOT NULL, timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, acknowledged_count BIGINT NOT NULL DEFAULT 0, INDEX idx_post_uid_time (ib_uid, timestamp), INDEX idx_post_parentid (parentid))",
  )
  .execute(pool)
  .await?;

  let ack_column_exists = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'post' AND COLUMN_NAME = 'acknowledged_count'"
    )
    .fetch_one(pool)
    .await?;

  if ack_column_exists == 0 {
    sqlx::query("ALTER TABLE post ADD COLUMN acknowledged_count BIGINT NOT NULL DEFAULT 0")
      .execute(pool)
      .await?;
  }

  // Add Full-Text Index to post table
  let _ = sqlx::query("ALTER TABLE post ADD FULLTEXT INDEX ft_post (post)")
    .execute(pool)
    .await;

  sqlx::query(
    "CREATE TABLE IF NOT EXISTS dm (id BIGINT PRIMARY KEY AUTO_INCREMENT, sender_uid BIGINT NOT NULL, recipient_uid BIGINT NOT NULL, message TEXT NOT NULL, created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, read_at TIMESTAMP NULL DEFAULT NULL, INDEX idx_dm_recipient_read (recipient_uid, read_at), INDEX idx_dm_pair_time (sender_uid, recipient_uid, created_at))",
  )
  .execute(pool)
  .await?;

  sqlx::query(
    "CREATE TABLE IF NOT EXISTS post_tag (id BIGINT PRIMARY KEY AUTO_INCREMENT, postid VARCHAR(64) NOT NULL, tag VARCHAR(64) NOT NULL, created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE KEY uniq_post_tag (postid, tag), INDEX idx_post_tag_created (created_at), INDEX idx_post_tag_tag (tag), INDEX idx_post_tag_postid (postid))",
  )
  .execute(pool)
  .await?;

  sqlx::query(
    "CREATE TABLE IF NOT EXISTS dm (id BIGINT PRIMARY KEY AUTO_INCREMENT, sender_uid BIGINT NOT NULL, recipient_uid BIGINT NOT NULL, message VARBINARY(1280) NOT NULL, created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, read_at TIMESTAMP NULL DEFAULT NULL, INDEX idx_dm_recipient_read (recipient_uid, read_at), INDEX idx_dm_pair_time (sender_uid, recipient_uid, created_at))",
  )
  .execute(pool)
  .await?;

  sqlx::query(
    "CREATE TABLE IF NOT EXISTS project_profile (id BIGINT PRIMARY KEY AUTO_INCREMENT, ib_uid BIGINT NOT NULL, project VARCHAR(255) NOT NULL, description TEXT NOT NULL, languages VARCHAR(255) NOT NULL, reinforcements VARCHAR(9999), reinforcements_request BOOLEAN DEFAULT FALSE, created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, INDEX idx_project_profile_uid_time (ib_uid, updated_at))",
  )
  .execute(pool)
  .await?;

  // Add reinforcements columns if they don't exist
  let _ = sqlx::query(
    "ALTER TABLE project_profile ADD COLUMN reinforcements VARCHAR(9999) DEFAULT NULL",
  )
  .execute(pool)
  .await;

  let _ = sqlx::query(
    "ALTER TABLE project_profile ADD COLUMN reinforcements_request BOOLEAN DEFAULT FALSE",
  )
  .execute(pool)
  .await;

  sqlx::query(
    "CREATE TABLE IF NOT EXISTS advert_image (imageid BIGINT PRIMARY KEY AUTO_INCREMENT, imagepath VARCHAR(1024) NOT NULL, url VARCHAR(2048) NOT NULL, owner_uid BIGINT NOT NULL DEFAULT 0, owner_username VARCHAR(255) NOT NULL DEFAULT '', paypal_order_id VARCHAR(128) NULL, payment_status VARCHAR(32) NOT NULL DEFAULT 'pending', clicks BIGINT NOT NULL DEFAULT 0, views BIGINT NOT NULL DEFAULT 0, created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP)",
  )
  .execute(pool)
  .await?;

  let owner_uid_exists = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'advert_image' AND COLUMN_NAME = 'owner_uid'"
    )
    .fetch_one(pool)
    .await?;
  if owner_uid_exists == 0 {
    sqlx::query("ALTER TABLE advert_image ADD COLUMN owner_uid BIGINT NOT NULL DEFAULT 0")
      .execute(pool)
      .await?;
  }

  let owner_username_exists = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'advert_image' AND COLUMN_NAME = 'owner_username'"
    )
    .fetch_one(pool)
    .await?;
  if owner_username_exists == 0 {
    sqlx::query("ALTER TABLE advert_image ADD COLUMN owner_username VARCHAR(255) NOT NULL DEFAULT ''")
      .execute(pool)
      .await?;
  }

  let paypal_order_id_exists = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'advert_image' AND COLUMN_NAME = 'paypal_order_id'"
    )
    .fetch_one(pool)
    .await?;
  if paypal_order_id_exists == 0 {
    sqlx::query("ALTER TABLE advert_image ADD COLUMN paypal_order_id VARCHAR(128) NULL")
      .execute(pool)
      .await?;
  }

  let payment_status_exists = sqlx::query_scalar::<_, i64>(
      "SELECT COUNT(*) FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'advert_image' AND COLUMN_NAME = 'payment_status'"
    )
    .fetch_one(pool)
    .await?;
  if payment_status_exists == 0 {
    sqlx::query("ALTER TABLE advert_image ADD COLUMN payment_status VARCHAR(32) NOT NULL DEFAULT 'pending'")
      .execute(pool)
      .await?;
  }

  Ok(())
}
pub async fn replace_post_tags(pool: &MySqlPool, postid: &str, post_text: &str) -> Result<(), sqlx::Error> {
  sqlx::query("DELETE FROM post_tag WHERE postid = ?")
    .bind(postid)
    .execute(pool)
    .await?;

  let tags = extract_hashtags(post_text);

  for tag in tags {
    sqlx::query("INSERT INTO post_tag (postid, tag, created_at) VALUES (?, ?, NOW())")
      .bind(postid)
      .bind(tag)
      .execute(pool)
      .await?;
  }

  Ok(())
}
pub async fn remove_post_tags(pool: &MySqlPool, postid: &str) -> Result<(), sqlx::Error> {
  sqlx::query("DELETE FROM post_tag WHERE postid = ?")
    .bind(postid)
    .execute(pool)
    .await?;

  Ok(())
}
pub async fn remove_post_acks(pool: &MySqlPool, postid: &str) -> Result<(), sqlx::Error> {
  sqlx::query("DELETE FROM post_ack WHERE postid = ?")
    .bind(postid)
    .execute(pool)
    .await?;

  Ok(())
}
pub async fn backfill_recent_post_tags(pool: &MySqlPool) -> Result<(), sqlx::Error> {
  let rows = sqlx::query_as::<_, RecentPostTagBackfillRow>(
      "SELECT postid, post FROM post WHERE `timestamp` >= (NOW() - INTERVAL 1 DAY)"
    )
    .fetch_all(pool)
    .await?;

  for row in rows {
    replace_post_tags(pool, &row.postid, &row.post).await?;
  }

  Ok(())
}

pub async fn lookup_profile_terms_by_uid(state: &AppState, uid: i64) -> Option<String> {
  let row = sqlx::query_as::<_, ProRow>(
      "SELECT ibp, pro, location, services, website, github FROM pro WHERE ib_uid = ? LIMIT 1"
    )
    .bind(uid)
    .fetch_optional(&state.db_pool)
    .await
    .ok()
    .flatten()?;

  let combined = format!("{} {}", row.pro, row.ibp);
  Some(combined.trim().to_string())
}
pub async fn lookup_following_usernames(state: &AppState, session_uid: Option<i64>) -> HashSet<String> {
  let Some(uid) = session_uid else {
    return HashSet::new();
  };

  let session_username = match sqlx::query_as::<_, SessionUserRow>(
    "SELECT username FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1",
  )
  .bind(uid.to_string())
  .fetch_optional(&state.db_pool)
  .await
  {
    Ok(Some(row)) if !row.username.trim().is_empty() => row.username,
    _ => return HashSet::new(),
  };

  let token = format!("%{}%", escape_mysql_like_token(&session_username.to_lowercase()));
  let candidate_rows = match sqlx::query_as::<_, FollowLookupRow>(
    "SELECT username, COALESCE(followers, '') AS followers FROM user WHERE LOWER(COALESCE(followers, '')) LIKE ? ESCAPE '\\\\'",
  )
  .bind(token)
  .fetch_all(&state.db_pool)
  .await
  {
    Ok(rows) => rows,
    Err(_) => return HashSet::new(),
  };

  let mut followed = HashSet::new();
  for row in candidate_rows {
    let is_followed = row
      .followers
      .split(',')
      .map(|value| value.trim())
      .filter(|value| !value.is_empty())
      .any(|value| value.eq_ignore_ascii_case(&session_username));

    if is_followed {
      followed.insert(row.username.to_lowercase());
    }
  }

  followed
}
pub async fn lookup_follower_usernames(state: &AppState, session_uid: Option<i64>) -> HashSet<String> {
  let Some(uid) = session_uid else {
    return HashSet::new();
  };

  let session_row = match sqlx::query_as::<_, FollowLookupRow>(
    "SELECT username, COALESCE(followers, '') AS followers FROM user WHERE CONVERT(ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = ? LIMIT 1",
  )
  .bind(uid.to_string())
  .fetch_optional(&state.db_pool)
  .await
  {
    Ok(Some(row)) => row,
    _ => return HashSet::new(),
  };

  session_row
    .followers
    .split(',')
    .map(|value| value.trim())
    .filter(|value| !value.is_empty())
    .map(|value| value.to_lowercase())
    .collect()
}
pub async fn load_project_profile_ack_map(state: &AppState, usernames: &HashSet<String>) -> HashMap<String, i64> {
  let mut ack_map = HashMap::new();

  for username in usernames {
    let normalized = username.trim().to_ascii_lowercase();
    if normalized.is_empty() || ack_map.contains_key(&normalized) {
      continue;
    }

    if let Ok(Some(row)) = sqlx::query_as::<_, UserHoverLookupRow>(
      "SELECT CONVERT(ib_uid USING utf8mb4) AS ib_uid, username, COALESCE(followers, '') AS followers, COALESCE(total_acknowledgments, 0) AS total_acknowledgments FROM user WHERE LOWER(username) = LOWER(?) LIMIT 1",
    )
    .bind(username)
    .fetch_optional(&state.db_pool)
    .await
    {
      ack_map.insert(normalized, row.total_acknowledgments);
    }
  }

  ack_map
}
pub async fn lookup_user_by_username(state: &AppState, username: &str) -> Result<Option<(i64, String)>, sqlx::Error> {
  let row = sqlx::query_as::<_, MessageUserLookupRow>(
      "SELECT CONVERT(ib_uid USING utf8mb4) AS ib_uid, username FROM user WHERE LOWER(username) = LOWER(?) LIMIT 1"
    )
    .bind(username)
    .fetch_optional(&state.db_pool)
    .await?;

  let parsed = row.and_then(|r| r.ib_uid.parse::<i64>().ok().map(|uid| (uid, r.username)));
  Ok(parsed)
}

pub async fn load_inbox_contacts(
  state: &AppState,
  ib_uid: i64,
  ib_user: &str,
) -> Result<Vec<String>, String> {
  let followers_row = sqlx::query_as::<_, FollowLookupRow>(
      "SELECT username, COALESCE(followers, '') AS followers FROM user WHERE LOWER(username) = LOWER(?) LIMIT 1"
    )
    .bind(ib_user)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| format!("Inbox followers lookup failed: {}", e))?;

  let mut inbox_users: Vec<String> = followers_row
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

  let conversation_rows = sqlx::query_as::<_, ConversationUsernameRow>(
      "SELECT DISTINCT CAST(COALESCE(CONVERT(counter.username USING utf8mb4), '') AS CHAR CHARACTER SET utf8mb4) AS username FROM dm AS dm LEFT JOIN user AS counter ON CONVERT(counter.ib_uid USING utf8mb4) COLLATE utf8mb4_unicode_ci = CAST(CASE WHEN dm.sender_uid = ? THEN dm.recipient_uid ELSE dm.sender_uid END AS CHAR CHARACTER SET utf8mb4) COLLATE utf8mb4_unicode_ci WHERE dm.sender_uid = ? OR dm.recipient_uid = ?"
    )
    .bind(ib_uid)
    .bind(ib_uid)
    .bind(ib_uid)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| format!("Inbox conversation lookup failed: {}", e))?;

  for row in conversation_rows {
    let username = row.username.trim();
    if username.is_empty() {
      continue;
    }

    if !inbox_users.iter().any(|existing| existing.eq_ignore_ascii_case(username)) {
      inbox_users.push(username.to_string());
    }
  }

  Ok(inbox_users)
}

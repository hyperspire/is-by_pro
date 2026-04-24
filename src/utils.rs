use crate::models::*;
use actix_web::HttpResponse;
use actix_web::HttpRequest;
use std::collections::HashSet;
use redis::AsyncCommands;

pub async fn get_cache(pool: &redis::aio::ConnectionManager, key: &str) -> Option<String> {
  let mut conn = pool.clone();
  conn.get(key).await.ok()
}

pub async fn set_cache(pool: &redis::aio::ConnectionManager, key: &str, value: &str, ttl_seconds: u64) {
  let mut conn = pool.clone();
  let _: Result<(), _> = conn.set_ex(key, value, ttl_seconds).await;
}

pub fn escape_html(input: &str) -> String {
  input
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;")
    .replace('\'', "&#39;")
}
pub fn escape_mysql_regex_token(input: &str) -> String {
  let mut escaped = String::with_capacity(input.len());

  for ch in input.chars() {
    match ch {
      '\\' | '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' => {
        escaped.push('\\');
        escaped.push(ch);
      }
      _ => escaped.push(ch),
    }
  }

  escaped
}
pub fn escape_mysql_like_token(input: &str) -> String {
  input
    .replace('\\', "\\\\")
    .replace('%', "\\%")
    .replace('_', "\\_")
}
pub fn url_encode_component(input: &str) -> String {
  let mut encoded = String::with_capacity(input.len());

  for byte in input.bytes() {
    if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
      encoded.push(byte as char);
    } else {
      encoded.push_str(&format!("%{:02X}", byte));
    }
  }

  encoded
}
pub fn normalize_hashtag(raw_tag: &str) -> Option<String> {
  let mut normalized = raw_tag.trim();

  if let Some(without_hash) = normalized.strip_prefix('#') {
    normalized = without_hash;
  }

  if normalized.is_empty() {
    return None;
  }

  if !normalized
    .chars()
    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
  {
    return None;
  }

  Some(normalized.to_lowercase())
}
pub fn trim_trailing_url_punctuation(raw_url: &str) -> (&str, &str) {
  let trimmed = raw_url.trim_end_matches(|ch: char| {
    matches!(ch, '.' | ',' | '!' | '?' | ':' | ';' | ')' | ']')
  });

  let suffix = &raw_url[trimmed.len()..];
  (trimmed, suffix)
}
pub fn extract_hashtags(raw_text: &str) -> Vec<String> {
  let chars: Vec<(usize, char)> = raw_text.char_indices().collect();
  let mut index = 0usize;
  let mut tags = Vec::new();
  let mut seen = HashSet::new();

  while index < chars.len() {
    let (byte_pos, ch) = chars[index];
    let previous_is_tag_char = index > 0
      && (chars[index - 1].1.is_ascii_alphanumeric() || chars[index - 1].1 == '_');

    if ch == '#' && !previous_is_tag_char {
      let mut tag_end_index = index + 1;

      while tag_end_index < chars.len() {
        let next_char = chars[tag_end_index].1;
        if next_char.is_ascii_alphanumeric() || next_char == '_' {
          tag_end_index += 1;
        } else {
          break;
        }
      }

      if tag_end_index > index + 1 {
        let tag_start_byte = byte_pos + '#'.len_utf8();
        let tag_end_byte = if tag_end_index < chars.len() {
          chars[tag_end_index].0
        } else {
          raw_text.len()
        };
        let tag_value = &raw_text[tag_start_byte..tag_end_byte];

        if let Some(normalized) = normalize_hashtag(tag_value) {
          if seen.insert(normalized.clone()) {
            tags.push(normalized);
          }
        }

        index = tag_end_index;
        continue;
      }
    }

    index += 1;
  }

  tags
}
pub fn extract_mentions(raw_text: &str) -> Vec<String> {
  let chars: Vec<(usize, char)> = raw_text.char_indices().collect();
  let mut index = 0usize;
  let mut mentions = Vec::new();
  let mut seen = HashSet::new();

  while index < chars.len() {
    let (byte_pos, ch) = chars[index];
    let previous_is_token_char = index > 0
      && (chars[index - 1].1.is_ascii_alphanumeric() || chars[index - 1].1 == '_');

    if ch == '@' && !previous_is_token_char {
      let mut mention_end_index = index + 1;

      while mention_end_index < chars.len() {
        let next_char = chars[mention_end_index].1;
        if next_char.is_ascii_alphanumeric() || next_char == '_' {
          mention_end_index += 1;
        } else {
          break;
        }
      }

      if mention_end_index > index + 1 {
        let mention_start_byte = byte_pos + '@'.len_utf8();
        let mention_end_byte = if mention_end_index < chars.len() {
          chars[mention_end_index].0
        } else {
          raw_text.len()
        };
        let mention_value = &raw_text[mention_start_byte..mention_end_byte];

        if seen.insert(mention_value.to_lowercase()) {
          mentions.push(mention_value.to_string());
        }

        index = mention_end_index;
        continue;
      }
    }

    index += 1;
  }

  mentions
}
pub fn highlight_terms(raw_text: &str, terms: &[String]) -> String {
  if terms.is_empty() {
    return escape_html(raw_text);
  }

  let chars: Vec<char> = raw_text.chars().collect();
  let lower_chars: Vec<char> = raw_text.to_lowercase().chars().collect();
  let n = chars.len();
  let mut matched = vec![false; n];

  for term in terms {
    if term.is_empty() {
      continue;
    }
    let term_chars: Vec<char> = term.to_lowercase().chars().collect();
    let tlen = term_chars.len();
    let mut i = 0;
    while i + tlen <= n {
      if lower_chars[i..i + tlen] == term_chars[..] {
        for j in i..i + tlen {
          matched[j] = true;
        }
        i += tlen;
      } else {
        i += 1;
      }
    }
  }

  let mut result = String::new();
  let mut in_match = false;
  let mut buffer = String::new();

  for (i, &ch) in chars.iter().enumerate() {
    if matched[i] && !in_match {
      result.push_str(&escape_html(&buffer));
      buffer.clear();
      in_match = true;
      buffer.push(ch);
    } else if !matched[i] && in_match {
      result.push_str("<strong>");
      result.push_str(&escape_html(&buffer));
      result.push_str("</strong>");
      buffer.clear();
      in_match = false;
      buffer.push(ch);
    } else {
      buffer.push(ch);
    }
  }

  if !buffer.is_empty() {
    if in_match {
      result.push_str("<strong>");
      result.push_str(&escape_html(&buffer));
      result.push_str("</strong>");
    } else {
      result.push_str(&escape_html(&buffer));
    }
  }

  result
}
pub fn get_rank_info(acks: i64) -> &'static RankInfo {
  for rank in RANK_TABLE {
    if acks >= rank.threshold {
      return rank;
    }
  }
  &RANK_TABLE[RANK_TABLE.len() - 1]
}
pub fn get_rank_asset(acks: i64) -> &'static str {
  get_rank_info(acks).asset
}
pub fn rank_from_unique_acknowledgments(total: i64) -> (i64, &'static str) {
  let rank = get_rank_info(total);
  (rank.level, rank.name)
}
pub async fn redirect_to_https(req: HttpRequest) -> HttpResponse {
  let host = req.connection_info().host().to_owned();
  let path = req.uri().path_and_query().map(|p| p.as_str()).unwrap_or("/");
  HttpResponse::MovedPermanently()
    .insert_header(("Location", format!("https://{}{}", host, path)))
    .finish()
}

pub fn is_mobile_device(req: &HttpRequest) -> bool {
  if let Some(user_agent) = req.headers().get("user-agent") {
    if let Ok(ua_str) = user_agent.to_str() {
      let ua_lower = ua_str.to_lowercase();
      ua_lower.contains("mobile") ||
      ua_lower.contains("android") ||
      ua_lower.contains("iphone") ||
      ua_lower.contains("ipad") ||
      ua_lower.contains("windows phone") ||
      ua_lower.contains("blackberry") ||
      ua_lower.contains("webos")
    } else {
      false
    }
  } else {
    false
  }
}

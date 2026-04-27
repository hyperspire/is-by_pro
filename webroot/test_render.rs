fn trim_trailing_url_punctuation(s: &str) -> (&str, &str) {
    let mut end = s.len();
    while end > 0 {
        let last_char = s[..end].chars().last().unwrap();
        if !last_char.is_ascii_punctuation() || last_char == '/' || last_char == '=' || last_char == '&' || last_char == '#' {
            break;
        }
        end -= last_char.len_utf8();
    }
    (&s[..end], &s[end..])
}

fn escape_html(s: &str) -> String {
    s.replace("&", "&amp;")
     .replace("<", "&lt;")
     .replace(">", "&gt;")
     .replace("\"", "&quot;")
     .replace("'", "&#x27;")
}

fn extract_rumble_info(url: &str) -> Option<String> {
    if let Some(pos) = url.find("rumble.com/embed/") {
        let start = pos + "rumble.com/embed/".len();
        let rest = &url[start..];
        let end = rest.find('/').unwrap_or(rest.len());
        let end = rest[..end].find('?').unwrap_or(end);
        let id = &rest[..end];
        if !id.is_empty() {
            return Some(format!("EMBED: {}", id));
        }
    } else if let Some(pos) = url.find("rumble.com/v") {
        let start = pos + "rumble.com/".len();
        let rest = &url[start..];
        if let Some(hyphen_pos) = rest.find('-') {
            let id = &rest[..hyphen_pos];
            if !id.is_empty() {
                return Some(format!("EMBED: {}", id));
            }
        }
    }
    None
}

pub fn render_post_with_hashtags(raw_text: &str) -> String {
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

      if let Some(rumble_html) = extract_rumble_info(&href) {
        rendered.push_str(&rumble_html);
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
    
    index += 1;
  }
  rendered.push_str(&escape_html(&raw_text[cursor..]));
  rendered
}

fn main() {
    println!("{}", render_post_with_hashtags("https://rumble.com/v78yg1a-hidden-hands-genetics-reptilians-and-the-patterns-behind-world-conflict.html"));
}

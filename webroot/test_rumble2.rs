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

fn main() {
    println!("{:?}", extract_rumble_info("https://rumble.com/v78yg1a-hidden-hands-genetics-reptilians-and-the-patterns-behind-world-conflict.html"));
}

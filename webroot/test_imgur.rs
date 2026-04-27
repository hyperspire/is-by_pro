fn extract_imgur_info(url: &str) -> Option<String> {
    if let Some(pos) = url.find("i.imgur.com/") {
        let rest = &url[pos + "i.imgur.com/".len()..];
        let end = rest.find('?').unwrap_or(rest.len());
        let path = &rest[..end];
        return Some(format!(r#"<div class="imgur-preview-wrapper" style="display:flex; justify-content:center; width:100%; margin: 10px 0;"><img src="https://i.imgur.com/{}" style="max-width: 100%; max-height: 500px; border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.1);" alt="Imgur Image"></div>"#, path));
    } else if let Some(pos) = url.find("imgur.com/") {
        let rest = &url[pos + "imgur.com/".len()..];
        let end = rest.find('?').unwrap_or(rest.len());
        let path = &rest[..end];
        let path = path.trim_end_matches('/');
        
        if path.is_empty() {
            return None;
        }

        if !path.starts_with("a/") && !path.starts_with("gallery/") {
            return Some(format!(r#"<div class="imgur-preview-wrapper" style="display:flex; justify-content:center; width:100%; margin: 10px 0;"><img src="https://i.imgur.com/{}.jpg" style="max-width: 100%; max-height: 500px; border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.1);" alt="Imgur Image"></div>"#, path));
        } else {
            return Some(format!(r#"<div class="imgur-preview-wrapper" style="display:flex; justify-content:center; width:100%; margin: 10px 0;"><iframe scrolling="no" src="https://imgur.com/{}/embed?pub=true" style="width: 100%; max-width: 560px; height: 500px; border: none; border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.1);" allowfullscreen="true"></iframe></div>"#, path));
        }
    }
    None
}

fn main() {
    println!("{:?}", extract_imgur_info("https://i.imgur.com/abcdef.jpg"));
    println!("{:?}", extract_imgur_info("https://imgur.com/abcdef"));
    println!("{:?}", extract_imgur_info("https://imgur.com/a/abcdef"));
}

fn escape_html(s: &str) -> String { s.to_string() }
fn extract_rumble_info(url: &str) -> Option<String> {
    if let Some(pos) = url.find("rumble.com/embed/") {
        let start = pos + "rumble.com/embed/".len();
        let rest = &url[start..];
        let end = rest.find('/').unwrap_or(rest.len());
        let end = rest[..end].find('?').unwrap_or(end);
        let id = &rest[..end];
        if !id.is_empty() {
            let original_url = format!("https://rumble.com/embed/{}/", id);
            return Some(format!(
                r#"<span class="youtube-rich-preview generic-link-preview-card" data-url="{}" style="margin: 10px 0; display: flex; flex-direction: column;">
                    <span style="width:100%; position: relative; overflow: hidden; padding-bottom: 56.25%; height: 0; display: block;">
                        <iframe src="https://rumble.com/embed/{}/" title="Rumble video player" allow="autoplay; encrypted-media; picture-in-picture" allowfullscreen width="100%" height="100%" style="border:0; position:absolute; top:0; left:0; width:100%; height:100%;"></iframe>
                    </span>
                    <span class="youtube-meta-container generic-link-preview-content" style="display: none;"></span>
                </span>"#,
                escape_html(&original_url),
                escape_html(id)
            ));
        }
    }
    None
}
fn main() {
    println!("{:?}", extract_rumble_info("https://rumble.com/embed/v783mw4/?pub=4"));
}

use pulldown_cmark::{Parser, Options, html, Event, Tag, TagEnd};
use regex::Regex;

fn main() {
    let raw_text = "Here is a URL: https://google.com/search?q=hello. And here is [a link](https://rust-lang.org).";
    let options = Options::all();
    let parser = Parser::new_ext(raw_text, options);
    
    let mut new_events = Vec::new();
    let mut in_link = false;
    let re = Regex::new(r"https?://[^\s<]+").unwrap();
    
    for event in parser {
        match event {
            Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
                in_link = true;
                new_events.push(Event::Start(Tag::Link { link_type, dest_url, title, id }));
            }
            Event::End(TagEnd::Link) => {
                in_link = false;
                new_events.push(Event::End(TagEnd::Link));
            }
            Event::Text(text) => {
                if in_link {
                    new_events.push(Event::Text(text));
                } else {
                    let mut last = 0;
                    for cap in re.captures_iter(&text) {
                        let m = cap.get(0).unwrap();
                        if m.start() > last {
                            new_events.push(Event::Text(text[last..m.start()].to_string().into()));
                        }
                        
                        let mut url = m.as_str();
                        let mut trailing_len = 0;
                        for c in url.chars().rev() {
                            if ['.', ',', '!', '?', ';', ':', ')', ']', '}'].contains(&c) {
                                trailing_len += c.len_utf8();
                            } else {
                                break;
                            }
                        }
                        let trailing = &url[url.len() - trailing_len..];
                        url = &url[..url.len() - trailing_len];
                        
                        let html = format!(r#"<a class="post-link" href="{url}" target="_blank" rel="noopener">{url}</a>"#);
                        new_events.push(Event::Html(html.into()));
                        if trailing_len > 0 {
                            new_events.push(Event::Text(trailing.to_string().into()));
                        }
                        
                        last = m.end();
                    }
                    if last < text.len() {
                        new_events.push(Event::Text(text[last..].to_string().into()));
                    }
                }
            }
            e => new_events.push(e),
        }
    }
    
    let mut html_output = String::new();
    html::push_html(&mut html_output, new_events.into_iter());
    println!("{}", html_output);
}

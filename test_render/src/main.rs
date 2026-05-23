use pulldown_cmark::{Parser, Options, Event, Tag, TagEnd, CodeBlockKind};
use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    static ref PROJECT_INVITE_REGEX: Regex = Regex::new(r":\[\[ :project-invite: (\d+) \]\]:").unwrap();
}

fn escape_html(s: &str) -> String {
    s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
}

fn render_post_with_hashtags(raw_text: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(raw_text, options);

    let mut merged_events: Vec<Event> = Vec::new();
    for event in parser {
        if let Event::Text(text) = &event {
            if let Some(Event::Text(last_text)) = merged_events.last_mut() {
                let mut new_string = last_text.to_string();
                new_string.push_str(text);
                *last_text = new_string.into();
                continue;
            }
        }
        merged_events.push(event);
    }

    let mut new_events = Vec::new();
    for event in merged_events {
        match event {
            Event::Text(text) => {
                new_events.push(Event::Text(text));

                let mut final_events = Vec::new();
                for ev in new_events {
                    if let Event::Text(t) = ev {
                        let t_str = t.into_string();
                        if PROJECT_INVITE_REGEX.is_match(&t_str) {
                            let mut last_inv = 0;
                            for inv_cap in PROJECT_INVITE_REGEX.captures_iter(&t_str) {
                                let inv_m = inv_cap.get(0).unwrap();
                                if inv_m.start() > last_inv {
                                    final_events.push(Event::Text(t_str[last_inv..inv_m.start()].to_string().into()));
                                }
                                let project_id = inv_cap.get(1).unwrap().as_str();
                                let html = format!(r#"<button class="post-submit" onclick="acceptProjectInvite({})">Accept Project Invite</button>"#, escape_html(project_id));
                                final_events.push(Event::Html(html.into()));
                                last_inv = inv_m.end();
                            }
                            if last_inv < t_str.len() {
                                final_events.push(Event::Text(t_str[last_inv..].to_string().into()));
                            }
                        } else {
                            final_events.push(Event::Text(t_str.into()));
                        }
                    } else {
                        final_events.push(ev);
                    }
                }
                new_events = final_events;
            }
            Event::Html(html) => {
                new_events.push(Event::Html(html.into()));
            }
            e => new_events.push(e),
        }
    }

    let mut rendered = String::new();
    pulldown_cmark::html::push_html(&mut rendered, new_events.into_iter());
    rendered
}

fn main() {
    let input = "I have invited you to collaborate on my project: **foo**. \n\n:[[ :project-invite: 1 ]]:";
    println!("{}", render_post_with_hashtags(input));
}

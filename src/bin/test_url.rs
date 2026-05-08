use pulldown_cmark::{Parser, Options, Event};

fn main() {
    let raw_text = "https://youtu.be/_fD1SBsM4LQ?si=1h85bPLIJw-ojs_u";
    let options = Options::all();
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
    
    for event in merged_events {
        println!("{:?}", event);
    }
}

use pulldown_cmark::{Parser, Options, Event, Tag, TagEnd};

fn main() {
    let raw_text = "Check this out: https://youtu.be/_fD1SBsM4LQ?si=1h85bPLIJw-ojs_u";
    let mut options = Options::all();
    let parser = Parser::new_ext(raw_text, options);
    
    for event in parser {
        println!("{:?}", event);
    }
}

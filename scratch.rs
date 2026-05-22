use pulldown_cmark::{Parser, Options, Event};

fn main() {
    let mut options = Options::all();
    let text = "https://youtu.be/--S0Gj1QWCI?si=CTt953YAMktKgE2l";
    let parser = Parser::new_ext(text, options);
    for ev in parser {
        println!("{:?}", ev);
    }
}

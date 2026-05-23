use pulldown_cmark::{Parser, html, Options};

fn main() {
    let raw_text = "I have invited you to collaborate on my project: **Test**. \n\n:[[ :project-invite: 123 ]]:";
    let parser = Parser::new_ext(raw_text, Options::all());
    for e in parser {
        println!("{:?}", e);
    }
}

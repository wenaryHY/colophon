use pulldown_cmark::{html, Options, Parser};

pub fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(markdown, options);
    let mut html_out = String::new();
    html::push_html(&mut html_out, parser);
    sanitize_html(&html_out)
}

pub fn sanitize_html(html: &str) -> String {
    let mut builder = ammonia::Builder::default();
    builder.add_tags(&["span", "mark"]);
    builder.add_tag_attributes("span", &["style"]);
    builder.add_tag_attributes("mark", &["style"]);
    builder.clean(html).to_string()
}

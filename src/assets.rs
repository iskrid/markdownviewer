pub static TEMPLATE_HTML: &str = include_str!("../assets/template.html");
pub static MERMAID_JS: &[u8] = include_bytes!("../assets/mermaid.min.js");
pub static STYLES_CSS: &str = include_str!("../assets/styles.css");

pub fn get_full_document(body_html: &str) -> String {
    let tmpl = TEMPLATE_HTML
        .replace("{{CONTENT}}", body_html)
        .replace("{{STYLES}}", STYLES_CSS)
        .replace(
            "<!-- MERMAID_JS -->",
            std::str::from_utf8(MERMAID_JS).unwrap_or_default(),
        );
    tmpl
}

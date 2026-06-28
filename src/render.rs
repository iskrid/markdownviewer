use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use comrak::adapters::SyntaxHighlighterAdapter;
use comrak::options::Plugins;
use comrak::Options;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Color, ThemeSet};
use syntect::html::{append_highlighted_html_for_styled_line, IncludeBackground};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use base64::{engine::general_purpose::STANDARD, Engine};

struct SyntectHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl SyntectHighlighter {
    fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }
}

impl SyntaxHighlighterAdapter for SyntectHighlighter {
    fn write_highlighted(
        &self,
        output: &mut dyn fmt::Write,
        lang: Option<&str>,
        code: &str,
    ) -> fmt::Result {
        let theme = self.theme_set.themes.get("InspiredGitHub").unwrap();
        let lang_str = match lang {
            Some(l) if !l.is_empty() => l.split_once(',').map(|(left, _)| left).unwrap_or(l),
            _ => "Plain Text",
        };

        let syntax = self
            .syntax_set
            .find_syntax_by_token(lang_str)
            .or_else(|| self.syntax_set.find_syntax_by_extension(lang_str))
            .or_else(|| self.syntax_set.find_syntax_by_first_line(code))
            .or_else(|| {
                if lang_str == "typescript" {
                    self.syntax_set.find_syntax_by_token("javascript")
                } else {
                    None
                }
            })
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut html = String::new();

        for line in LinesWithEndings::from(code) {
            match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(regions) => {
                    let bg = theme
                        .settings
                        .background
                        .map(|c| (c.r as u32) << 16 | (c.g as u32) << 8 | (c.b as u32))
                        .unwrap_or(0xf6f8fa);
                    append_highlighted_html_for_styled_line(
                        &regions[..],
                        IncludeBackground::IfDifferent(Color {
                            r: ((bg >> 16) & 0xff) as u8,
                            g: ((bg >> 8) & 0xff) as u8,
                            b: (bg & 0xff) as u8,
                            a: 0xff,
                        }),
                        &mut html,
                    )
                    .map_err(|_| fmt::Error)?;
                }
                Err(_) => return output.write_str(code),
            };
        }

        output.write_str(&html)
    }

    fn write_pre_tag(
        &self,
        output: &mut dyn fmt::Write,
        attributes: HashMap<&'static str, Cow<'_, str>>,
    ) -> fmt::Result {
        let theme = self.theme_set.themes.get("InspiredGitHub").unwrap();
        let bg = theme.settings.background.unwrap_or(Color {
            r: 0xf6,
            g: 0xf8,
            b: 0xfa,
            a: 0xff,
        });

        output.write_str("<pre")?;
        for (key, value) in attributes.iter() {
            output.write_str(" ")?;
            if *key == "style" {
                let bg_style = format!("background-color:#{:02x}{:02x}{:02x};", bg.r, bg.g, bg.b);
                output.write_str(&format!("{}=\"{}{}\"", key, bg_style, value))?;
            } else {
                output.write_str(&format!("{}=\"{}\"", key, escape_html(value)))?;
            }
        }
        output.write_str(">")
    }

    fn write_code_tag(
        &self,
        output: &mut dyn fmt::Write,
        attributes: HashMap<&'static str, Cow<'_, str>>,
    ) -> fmt::Result {
        output.write_str("<code")?;
        for (key, value) in attributes.iter() {
            output.write_str(" ")?;
            output.write_str(&format!("{}=\"{}\"", key, escape_html(value)))?;
        }
        output.write_str(">")
    }
}

fn rewrite_relative_urls(html: &str, base_dir: Option<&Path>) -> String {
    let Some(base) = base_dir else {
        return html.to_string();
    };

    let re_url = regex::Regex::new(r#"(?P<attr>src|href)\s*=\s*"(?P<path>[^"]*)""#).unwrap();

    let result = re_url
        .replace_all(html, |caps: &regex::Captures| {
            let attr_name = &caps["attr"];
            let path = &caps["path"];

            if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("#")
            {
                format!(r#"{}="{}""#, attr_name, path)
            } else {
                let full_path = base.join(path);

                let abs_path = match full_path.canonicalize() {
                    Ok(p) => p,
                    Err(_) => full_path,
                };

                match std::fs::read(&abs_path) {
                    Ok(bytes) => {
                        let ext = abs_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                        let content_type = match ext {
                            "png" => "image/png",
                            "jpg" | "jpeg" => "image/jpeg",
                            "gif" => "image/gif",
                            "svg" => "image/svg+xml",
                            "webp" => "image/webp",
                            "ico" => "image/x-icon",
                            "bmp" => "image/bmp",
                            "tiff" | "tif" => "image/tiff",
                            "pdf" => "application/pdf",
                            "html" | "htm" => "text/html",
                            "css" => "text/css",
                            "js" => "application/javascript",
                            "json" => "application/json",
                            "xml" => "application/xml",
                            "txt" => "text/plain",
                            _ => "application/octet-stream",
                        };
                        let encoded = STANDARD.encode(&bytes);
                        let data_uri = format!("data:{};base64,{}", content_type, encoded);
                        format!(r#"{}="{}""#, attr_name, data_uri)
                    }
                    Err(_) => {
                        format!(r#"{}="{}""#, attr_name, path)
                    }
                }
            }
        })
        .to_string();

    result
}

fn transform_mermaid_blocks(html: &str) -> String {
    let re = regex::Regex::new(
        r#"(?s)<pre[^>]*><code[^>]*class="[^"]*language-mermaid[^"]*">(.*?)</code>\s*</pre>"#,
    )
    .unwrap();

    let result = re
        .replace_all(html, |caps: &regex::Captures| {
            let raw = &caps[1];
            let stripped = strip_html_tags(raw);
            let decoded = decode_html_entities(&stripped);
            format!(r#"<pre class="mermaid">{}</pre>"#, escape_html(&decoded))
        })
        .to_string();

    result
}

fn decode_html_entities(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

fn strip_html_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
    }
    result
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn render(md_text: &str, base_dir: Option<&Path>) -> String {
    let mut opts = Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.tasklist = true;
    opts.render.hardbreaks = false;

    let adapter = SyntectHighlighter::new();

    let mut plugins = Plugins::default();
    plugins.render.codefence_syntax_highlighter = Some(&adapter);

    let html = comrak::markdown_to_html_with_plugins(md_text, &opts, &plugins);

    let with_urls = rewrite_relative_urls(&html, base_dir);

    transform_mermaid_blocks(&with_urls)
}

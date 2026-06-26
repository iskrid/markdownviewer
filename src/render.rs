use std::collections::HashMap;
use std::path::Path;

static DEFAULT_SYNTAX_THEME: &[u8] = syntect::default_themes::LIGHT;

fn get_syntax_theme() -> syntect::LoadingResult<syntect::HighlightingTheme> {
    syntect::parsing::SyntaxSet::load_defaults_newlines();
    syntect::highlighting::ThemeSet::load_defaults_newlines();
    let themes = syntect::highlighting::ThemeSet::load_defaults_newlines();
    Ok(themes.themes["base16-light"].clone())
}

fn highlight_block(
    code: &str,
    language_hint: Option<&str>,
    syntax_set: &syntect::parsing::SyntaxSet,
    theme: &syntect::highlighting::HighlightingTheme,
) -> syntect::LoadingResult<String> {
    let syntax = if let Some(lang) = language_hint {
        syntax_set.find_syntax_by_token(lang).or_else(|| syntax_set.find_syntax_by_extension(lang))
    } else {
        None
    };

    match syntax {
        None => Ok(code.to_owned()),
        Some(syntax) => {
            let parsed = syntect::easy::highlight_from_reader(
                code.as_bytes(),
                syntax_set,
                syntax,
                theme,
            )?;
            Ok(String::from_utf8_lossy(&parsed).into_owned())
        }
    }
}

struct SyntaxHighlighterAdapter {
    syntax_set: syntect::parsing::SyntaxSet,
    theme: syntect::highlighting::HighlightingTheme,
}

impl SyntaxHighlighterAdapter {
    fn new() -> Self {
        let syntax_set = syntect::parsing::SyntaxSet::load_defaults_newlines();
        let themes = syntect::highlighting::ThemeSet::load_defaults_newlines();
        let theme = themes.themes["base16-light"].clone();
        Self { syntax_set, theme }
    }
}

impl comrak::plugin::format::html::CodeOutput {
    fn to_highlighted(
        code: &str,
        language_hint: Option<&str>,
        adapter: &SyntaxHighlighterAdapter,
    ) -> String {
        highlight_block(code, language_hint, &adapter.syntax_set, &adapter.theme).unwrap_or_else(|_| code.to_owned())
    }
}

struct SyntectHighlighter;

impl comrak::plugin::format::html::CodeOutput for SyntectHighlighter {
    fn get_highlighted_code(
        &self,
        syntax: Option<&str>,
        value: Vec<u8>,
        _code_str_without_backticks: &str,
    ) -> String {
        let code = String::from_utf8_lossy(&value);
        highlight_block(
            &code,
            syntax,
            &load_default_syntax_set(),
            &get_light_theme(),
        ).unwrap_or_else(|_| code.to_string())
    }
}

fn load_markdown_to_html(md: &str) -> String {
    comrak::markdown_to_html(md, &comrak::Options::default())
}

pub fn render(md_text: &str, base_dir: Option<&Path>) -> String {
    let mut opts = comrak::Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.tasklist = true;
    opts.extension.tagfilter = false;
    opts.render.hardbreaks = false;
    opts.render.github_pre_lang = false;

    let adapter = SyntaxHighlighterAdapter::new();
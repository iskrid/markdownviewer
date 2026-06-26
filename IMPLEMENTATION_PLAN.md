# Implementation Plan: `markdownviewer`

A minimal, fast Markdown viewer GUI for Linux (cross-platform-capable via
wry/tao). This document is the canonical design reference for the project.

---

## 1. Goals & requirements

### Functional
- Render a Markdown file as HTML in a desktop webview window.
- Syntax-highlight fenced code blocks (language-aware).
- Render Mermaid diagrams defined in ```` ```mermaid ```` blocks.
- Accept input from a file path **or** stdin (`markdownviewer -`).
- Follow relative links to other `.md` files (opens in the same window).
- Render relative images (e.g. `./img.png`) from the source file's directory.
- Open external `http(s)` links in the user's default browser.
- Live reload: re-render when the source file changes on disk.

### Non-functional
- **Short startup time.** Cold start should be dominated by the unavoidable
  WebKitGTK init, not by our code. Target: well under 500ms on a warm system.
- **Clean UI.** No menus, toolbar, or status bar. Just the document. Window
  title = source filename.
- **Small, self-contained binary.** All assets embedded; no runtime file
  dependencies. Pure-Rust where possible (no C libs to install for the build).
- **Linux-first, cross-platform optional.** Uses wry/tao which abstract the
  platform; v1 targets Linux but nothing prevents macOS/Windows builds later.

### Explicitly out of scope
Menus, toolbars, tabs, multiple windows, an editor, TUI mode, plugin system,
and server-side Mermaid rendering (mermaid.js runs client-side in the webview).

---

## 2. Library selection

Researched and chosen to minimize dependency count and compile time while
satisfying every requirement.

### Window + WebView: `tao` + `wry` (direct, NOT Tauri)

`tao` (tauri-apps) handles window creation and the event loop. `wry`
(tauri-apps) wraps the platform webview: WebKitGTK on Linux, WebView2 on
Windows, WKWebView on macOS.

**Why not full Tauri?** Tauri adds an IPC framework, plugin system, asset
bundler, and CLI tooling. For a single-window viewer that loads one HTML
string, all of that is overhead. Using `wry` directly keeps the dependency
tree small, the binary lean, and cold start fast. wry's `WebViewBuilder` gives
us everything we need: `with_html()` for inline content, `with_ipc_handler()`
for JS→Rust messages, and `with_custom_protocol()` for serving local assets.

Startup notes: WebKitGTK has a fixed init cost (~100-300ms) that is the floor
for *any* webview app on Linux. Our job is to make everything else fast enough
that this floor dominates.

### Markdown → HTML: `comrak` (custom syntect adapter)

`comrak` is a 100% CommonMark + GFM-compatible parser. It exposes a
`SyntaxHighlighterAdapter` trait (in `comrak::adapters`, **not** behind a
feature gate) that we implement ourselves using syntect. We pass our adapter
to `Plugins.render.codefence_syntax_highlighter` and call
`markdown_to_html_with_plugins` — the resulting HTML has highlighted
`<pre><code>` blocks with inline styles. No manual code-block extraction.

**Why not pulldown-cmark?** pulldown-cmark is a pull-parser (event iterator);
to get highlighted HTML you must walk events, intercept `CodeBlock` events,
run syntect yourself, and splice the result back into the HTML stream. comrak
with its plugin adapter does this in one call. comrak also has full GFM
support (tables, tasklists, strikethrough, autolinks) that pulldown-cmark
partially gates behind options.

**Feature flags:** comrak's default features pull in CLI tooling (`clap`,
`shell-words`, `xdg`, `fmt2io`, `bon`) AND a `syntect` feature that provides a
convenience `SyntectAdapter` struct. We set `default-features = false` and
enable **no** features. We do NOT enable comrak's `syntect` feature because it
pulls syntect with its default features (`default-onig`), and Cargo feature
unification would then force the `onig` (C oniguruma) dependency on us even
though our own syntect dep uses `default-fancy`. Instead, we implement the
`SyntaxHighlighterAdapter` trait ourselves — it's a small trait (three methods:
`write_highlighted`, `write_pre_tag`, `write_code_tag`) — using our own
pure-Rust syntect instance. This keeps the build 100% pure-Rust with no C
toolchain required.

### Code highlighting: `syntect` (`default-fancy`)

`syntect` uses Sublime Text syntax definitions and themes. It is the highlighter
used by `bat`, `mdcat`, and Typst. We instantiate our custom
`SyntaxHighlighterAdapter` with a GitHub-Light theme to match the viewer's
light theme.

**Why `default-fancy` instead of the default `default-onig`?** The default
features use `onig` (the oniguruma C library), which requires a system C
dependency and slows compilation. `default-fancy` uses `fancy-regex` — a
pure-Rust regex engine — so the entire build is pure-Rust with no C toolchain
required. We add `syntect` as a direct dependency with
`default-features = false, features = ["default-fancy"]` and implement comrak's
adapter trait ourselves to bridge it in (see the comrak section above for why
we can't use comrak's built-in `syntect` feature).

### Mermaid: `mermaid.js` v11 (embedded, client-side)

Mermaid is a JavaScript library that renders diagrams (flowcharts, sequence
diagrams, Gantt, git graphs, etc.) from a Markdown-ish text syntax. The
standalone `mermaid.min.js` bundle runs **fully offline** with no network
access and no server-side component.

We download `mermaid.min.js` v11 once into `assets/` and embed it into the
binary via `include_bytes!`. It is inlined as a `<script>` block in the HTML
template. `mermaid.initialize({startOnLoad:true})` finds every
`<pre class="mermaid">` and renders it to SVG in-place.

**Why client-side in the webview?** Server-side Mermaid rendering requires
puppeteer/headless Chromium — a huge dependency that defeats the
short-startup/small-binary goals. Letting WebKitGTK run the JS bundle is free
(we already have a webview) and keeps the Rust side simple.

### File watching: `notify-debouncer-mini`

Wraps the `notify` crate with built-in debouncing (~200ms). We watch the
source file (only when input was a file, not stdin) and re-render on change.

### External links: `webbrowser`

Cross-platform "open URL in default browser" — calls `xdg-open` on Linux.
Used by the IPC handler when the user clicks an external link.

### Config paths: `dirs`

Tiny crate for platform config-dir lookup. We store the persisted window size
at `~/.config/markdownviewer/size` on Linux.

### What we deliberately do NOT add
- `clap` — one positional argument (file path or `-`); hand-rolled parsing is
  a few lines and avoids a sizable dep.
- `serde` / `serde_json` — our IPC messages are two fixed shapes; we hand-build
  and hand-parse the small JSON strings.
- `reqwest` / any HTTP client — no network calls; mermaid is embedded, links
  go to the system browser.
- Any async runtime — the app is single-threaded event-loop driven by tao.

---

## 3. Project layout

```
markdownviewer/
  Cargo.toml              - manifest; release profile tuned for size/startup
  AGENTS.md               - agent/human contributor guide
  IMPLEMENTATION_PLAN.md  - this file
  src/
    main.rs     - entry: arg parse, read input, build window + webview, event loop, key handling
    render.rs   - md -> HTML: comrak + syntect, mermaid-block transform, relative-URL rewriting
    ipc.rs      - message handling (external_link, open_md) from JS bridge
    config.rs   - load/save window size to ~/.config/markdownviewer/size
    assets.rs   - include_str!/include_bytes! of template.html, mermaid.min.js, styles.css
  assets/
    template.html  - shell: <div id="content">, inlined mermaid.min.js, init + IPC bridge script
    mermaid.min.js - mermaid.js v11 standalone bundle (embedded into binary)
    styles.css     - GitHub-light styling for prose + code blocks + mermaid containers
```

---

## 4. Module designs

### `main.rs` — entry point and event loop

Responsibilities:
1. Parse argv: exactly one positional — a file path, or `-` for stdin.
   - No path / `-`: read all of stdin into a `String`.
   - File path: read the file into a `String`; record the canonical path
     (used as the base dir for resolving relative links/images).
   - Usage errors: print a one-line message to stderr and `exit(1)`.
2. Load persisted window size via `config::load_size()` (default 900x700).
3. Build the initial HTML document via `render::render_document(&md, &base_dir)`.
4. Create the `tao` `EventLoop` and `Window` (title = filename or "stdin").
5. Build the `wry` `WebView` on the window:
   - `.with_html(full_document)`
   - `.with_ipc_handler(ipc::handle)` — receives JSON from the JS bridge
   - `.with_custom_protocol("md", asset_protocol)` — serves local files
6. If input was a file, spawn the `notify-debouncer-mini` watcher on it. On a
   debounced event, re-read the file, re-render, and call
   `webview.eval("replaceContent('...')")` to swap the document. The eval
   string is JSON-escaped; the JS side updates `#content` and re-runs
   `mermaid.run()`.
7. Run the tao event loop. Handle `WindowEvent::KeyboardInput`:
   - `Esc` (press) → `window.set_minimized` no — `event_loop.exit()`
   - `Ctrl+W` (press) → `event_loop.exit()`
8. On `WindowEvent::CloseRequested` or loop exit, save the current window size
   via `config::save_size(width, height)`.

Notes:
- `panic = "abort"` is set in the release profile, so do not rely on unwinding.
  Keep I/O on `Result` and exit cleanly from `main` on error.
- The webview and window handles need to be accessible from the watcher
  callback and the IPC handler. Use `EventLoop` user events or `Arc`-shared
  handles per wry/tao patterns. The watcher runs on a background thread; it
  posts a user event to the event loop, and the main thread does the
  re-render + eval (wry eval must be called from the main thread).

### `render.rs` — markdown → full HTML document

Public API:
```rust
pub fn render_document(markdown: &str, base_dir: &Path) -> String;
```

Steps:
1. Build a `comrak::Arena` and `comrak::parse_document` with GFM options
   (tables, tasklists, strikethrough, autolinks enabled). Or use
   `markdown_to_html_with_plugins` for the simpler single-call path.
2. Implement a custom `SyntaxHighlighterAdapter` that wraps a syntect
   `SyntaxSet` (default syntaxes) and `ThemeSet` (GitHub Light theme). The
   adapter's `write_highlighted` method: look up the syntax by language name,
   use `HighlightLines::new`, iterate lines with `HighlightedLines::highlight`,
   and emit `<span style="color:...">` tags via `as_24_bit_terminal_escaped` or
   by formatting each `Style` as inline CSS. `write_pre_tag` and `write_code_tag`
   emit standard `<pre>` / `<code>` tags (optionally with a `class` attribute).
   Build the SyntaxSet/ThemeSet once (lazy static or passed in) and reuse.
3. Render to HTML via `markdown_to_html_with_plugins` with our adapter set on
   `plugins.render.codefence_syntax_highlighter`. Fenced code blocks become
   `<pre><code style="color: ...">...</code></pre>` with per-token inline styles.
4. Post-process the HTML string:
   - **Mermaid transform:** find `<pre><code class="language-mermaid">…</code></pre>`
     and rewrite to `<pre class="mermaid">…</pre>` (un-escape the inner text so
     mermaid.js sees raw diagram source). A targeted string/regex replace is
     fine; the comrak output is well-formed enough for this.
   - **Relative-URL rewrite:** for every `src="…"` and `href="…"` that is a
     relative path (not `http:`, `https:`, `mailto:`, `#`, `md:`), resolve it
     against `base_dir` and rewrite to `md://asset/<absolute-path>`. This makes
     local images loadable and lets the IPC handler recognize `.md` link clicks
     (it sees the absolute path and can open the file).
5. Inject the processed HTML body into `assets::TEMPLATE_HTML` at the
   `{{content}}` marker (or splice into the `<div id="content">` element).
   Return the full document string.

Why post-process instead of a custom comrak adapter? The mermaid transform is
a single language-tag substitution and the URL rewrite is a flat string scan;
both are simpler and more robust than implementing custom comrak adapters for
what amounts to two small, well-defined edits.

### `ipc.rs` — Rust ↔ JS message handling

```rust
pub fn handle(webview: &WebView, msg: &str);
```

Receives the raw JSON string posted by `window.ipc.postMessage(str)` in JS.
Parse the `type` field:

- `"external_link"` → `webbrowser::open(&payload.url)`. Ignore errors (best
  effort). Never navigate the webview to external URLs.
- `"open_md"` → `payload.path` is an absolute path (we rewrote it during
  render). Read the file, re-render with the new file's dir as base, and
  `webview.eval("replaceContent('...')")`. Also update the window title.
  Update the file watcher to follow the new file (stop watching the old one).

Parsing: the message shapes are fixed and tiny. Hand-parse with a couple of
`str::find`/substring calls or a minimal JSON helper. No `serde` dependency.

### `config.rs` — window-size persistence

```rust
pub fn load_size() -> (u32, u32);            // default 900x700 on any error
pub fn save_size(width: u32, height: u32);   // best-effort; ignore errors
```

- Config dir: `dirs::config_dir()` / `markdownviewer` / `size`.
- File format: a single line `width height\n` (ASCII digits, space-separated).
- `load_size` reads the file, splits on whitespace, parses two `u32`s. Any
  error (missing file, unparseable, IO error) → return the 900x700 default.
- `save_size` writes the line. Create the dir if needed. Never returns an
  error to the caller — persistence is best-effort.

### `assets.rs` — embedded assets

```rust
pub const TEMPLATE_HTML: &str = include_str!("../assets/template.html");
pub const MERMAID_JS: &str = include_str!("../assets/mermaid.min.js");
pub const STYLES_CSS: &str = include_str!("../assets/styles.css");
```

All three are baked into the binary at compile time. The template HTML inlines
`MERMAID_JS` and `STYLES_CSS` via `<style>` and `<script>` tags so the webview
needs zero external resource fetches. (`include_str!` is fine for
`mermaid.min.js` — it's valid UTF-8 text JS.)

### `assets/template.html`

Shell document. Structure:
```html
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>markdownviewer</title>
  <style>/* STYLES_CSS inlined here by assets.rs concatenation */</style>
  <script>/* MERMAID_JS inlined here */</script>
</head>
<body>
  <div id="content">{{content}}</div>
  <script>
    // IPC bridge + mermaid init
  </script>
</body>
</html>
```

The trailing `<script>` (in the template, not embedded) does:
1. `mermaid.initialize({ startOnLoad: true });` — renders `<pre class="mermaid">`
   on first load.
2. Register a click handler on `document` that intercepts `<a>` clicks:
   - If `href` starts with `http://` / `https://` / `mailto:` →
     `window.ipc.postMessage(JSON.stringify({type:"external_link",url:href}))`
     and `preventDefault()`.
   - If `href` resolves to a `.md` file (the URL was rewritten to
     `md://asset/<path>.md`) →
     `window.ipc.postMessage(JSON.stringify({type:"open_md",path:<path>}))`
     and `preventDefault()`.
   - Otherwise let the webview handle it (e.g. in-page `#anchor` jumps).
3. Expose `window.replaceContent = function(html) { ... }`:
   - Sets `document.getElementById("content").innerHTML = html`.
   - Calls `mermaid.run()` to render any new `<pre class="mermaid">` blocks.
   Used by the live-reload and `open_md` paths.

### `assets/styles.css`

GitHub-light-inspired styling:
- Body: system UI font stack, `max-width: 800px`, `margin: 2rem auto`,
  `line-height: 1.6`, comfortable padding.
- Headings, lists, tables, blockquotes: GitHub-ish spacing and borders.
- Code blocks: syntect emits inline-styled spans, so we only style the
  `<pre>` container (background `#f6f8fa`, border-radius, padding, overflow-x).
- Mermaid containers: `text-align: center` so diagrams center in the column.
- Light scrollbars (WebKitGTK respects `::-webkit-scrollbar` styling).

---

## 5. Custom protocol: `md://asset/<absolute-path>`

Registered via `WebViewBuilder::with_custom_protocol("md", handler)`.

Handler:
1. Parse the request URI; extract the path segment after `md://asset/`.
2. Read the file bytes from disk.
3. Sniff a MIME type from the extension (`.png`→`image/png`, `.jpg`→`image/jpeg`,
   `.svg`→`image/svg+xml`, `.gif`→`image/gif`, `.webp`→`image/webp`, fallback
   `application/octet-stream`). A small match on the lowercased extension.
4. Return `wry::http::Response::builder()` with the bytes, the content type,
   and `Access-Control-Allow-Origin: *` (avoids CORS issues in the webview).

This is what lets relative image references in the markdown actually render.

---

## 6. Live reload

Only active when input was a file (not stdin — stdin has no path to watch).

- `notify-debouncer-mini` with a ~200ms debounce window watches the source
  file's path. Multiple rapid saves collapse into one re-render.
- The watcher runs on a background thread. On a debounced event, it posts a
  tao `UserEvent` to the event loop (via `EventLoopProxy`). The main thread
  handles the event: re-reads the file, re-renders, and calls
  `webview.eval("replaceContent('...')")`.
- `replaceContent` (defined in the template) swaps `#content`'s innerHTML and
  re-runs `mermaid.run()` so new/changed diagrams render.
- The eval string is JSON-escaped to be safe inside a JS single-quoted string
  literal.

---

## 7. Keyboard handling

Handled in Rust (not JS) via tao's `WindowEvent::KeyboardInput`:
- `Esc` (key press) → exit the event loop → window closes.
- `Ctrl+W` (key press) → exit the event loop → window closes.

Nothing else. No reload key, no zoom keys, no find. Keeping the surface tiny.

---

## 8. Window-size persistence

- On open: `config::load_size()` → `(width, height)`, default `(900, 700)`.
  Used as the window's inner (or outer — pick one and document) size.
- On close (or event-loop exit): read the current window size from tao and
  call `config::save_size(w, h)`.
- File: `~/.config/markdownviewer/size`, contents `width height\n`.
- Best-effort: never let a persistence failure prevent startup or closing.

---

## 9. Cross-platform notes

v1 targets Linux (WebKitGTK). The chosen crates are all cross-platform, so a
future macOS/Windows build is mostly free:

- `wry`/`tao`: abstract the platform. `with_html`, `with_ipc_handler`, and
  `with_custom_protocol` work on all three.
- `webkit2gtk-4.1` is Linux-only; on Windows the dep is WebView2 (Edge), on
  macOS WKWebView. These are resolved by wry's feature flags, not our code.
- `webbrowser::open` already dispatches to the right OS mechanism.
- `dirs::config_dir()` already returns the platform-correct location.
- The only Linux-specific assumption is `xdg-open` existing, but that's
  `webbrowser`'s concern, not ours.
- Avoid `#[cfg(target_os)]` unless adding a genuinely platform-specific code
  path. None are expected for v1.

---

## 10. Release profile

```toml
[profile.release]
lto = true          # whole-program LTO for a tighter binary
codegen-units = 1   # best optimization (slower compile, fine for release)
strip = true        # strip symbols → smaller binary
opt-level = "z"     # optimize for size
panic = "abort"     # no unwinding machinery; smaller binary + faster startup
```

Rationale: a viewer launches often and loads once; runtime CPU is dominated
by WebKitGTK, not our code. So we optimize for **binary size and startup**,
not throughput. `lto` + `codegen-units=1` + `opt-level="z"` + `strip` gives
the smallest self-contained binary. `panic = "abort"` removes the unwinding
tables — keep fallible paths on `Result` and exit cleanly from `main`.

---

## 11. Startup-time budget

| Stage                              | Estimated cost        | Controllable? |
|------------------------------------|-----------------------|---------------|
| Process start + Rust runtime init  | ~5-15ms               | Minimal       |
| tao event loop + window creation   | ~20-50ms              | Some          |
| WebKitGTK webview init             | ~100-300ms            | No (floor)    |
| comrak + syntect setup (lazy)      | ~10-30ms              | Yes (lazy)    |
| HTML build (typical doc)           | ~1-5ms                | Yes           |
| `with_html` load + mermaid init    | ~20-80ms (diagram-dependent) | Yes     |
| **Total cold start (target)**      | **< 500ms**           |               |

Measures we take:
- All assets embedded → zero fs reads at startup.
- Single `with_html` call → one webview load, no resource round-trips.
- syntect SyntaxSet/ThemeSet built once and reused.
- LTO + size optimization → smaller binary maps faster.
- No async runtime, no plugin framework, no extra threads at startup (the
  file watcher spawns lazily after the window is up).

---

## 12. Build & run

```bash
# Quick type-check during development
cargo check

# Lint / format
cargo clippy --all-targets -- -D warnings
cargo fmt --check   # or: cargo fmt

# The artifact users run
cargo build --release

# Run against a markdown file
./target/release/markdownviewer README.md

# Run from stdin
cat foo.md | ./target/release/markdownviewer -
echo "# Hello" | ./target/release/markdownviewer -
```

### Runtime prerequisites (Linux)
- `webkit2gtk-4.1` (the webview engine)
- A running X11 or Wayland session
- `xdg-open` (only needed when the user clicks an external link)

---

## 13. Verification before declaring done

A task is not complete until all of these pass:
1. `cargo fmt --check` (apply with `cargo fmt` if it fails)
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo build --release`
4. Manual smoke test: run the viewer on a markdown file containing
   (a) a fenced ```` ```rust ```` code block, (b) a ```` ```mermaid ```` flowchart,
   (c) a relative image, and confirm all three render correctly.
5. Manual smoke test: edit the file in another editor and confirm the viewer
   updates within ~200ms (live reload).
6. Manual smoke test: click an external `https://` link and confirm it opens
   in the default browser; click a `.md` link and confirm the viewer swaps to
   that file.

---

## 14. Implementation status (2025-06-27)

### Done
- **Build compiles + clippy clean.** Rust ≥ 1.85, cargo fmt/clippy/release all pass.
- **Markdown loading.** File path (positional arg) and stdin (`markdownviewer -`) both work. Raw markdown printed to stderr for debugging during development.
- **HTML rendering via comrak.** GFM extensions enabled (tables, tasklists, strikethrough). Output printed to stderr; validated on AGENTS.md with headings/paragraphs/table producing correct HTML structure.
- **Template + assets embedded.** `template.html`, `mermaid.min.js` (real v10 standalone ~3.3 MB), `styles.css` compiled into binary via `include_str!`/`include_bytes!`. Final document size verified (3.3 MB dominated by mermaid).
- **Window creation via tao + webview.** Tao 0.35 event loop + WindowBuilder. On Linux uses `window.default_vbox()` as the GTK container for `.build_gtk()`, matching wry's own example pattern and avoiding the "GtkApplicationWindow can only contain one widget" conflict that occurs when passing `gtk_window()` directly.

### Remaining
- **IPC bridge** — no JS click interception, external link handling, or md-link cross-reference yet (template needs `<script>` with IPC + document click handler).
- **Custom protocol (`md://asset/`)** — relative image rendering not wired up (`.with_custom_protocol()` handler required in wry builder).
- **Syntax highlighting** — comrak currently has no `SyntaxHighlighterAdapter`; fenced code blocks render as plain `<pre><code>` without color. Need to implement the adapter trait with syntect integration per plan §3/render.rs design.
- **Mermaid transform** — post-process step that converts `<pre><code class="language-mermaid">` → `<pre class="mermaid">` not implemented in render pipeline yet.
- **File watching + live reload** — `notify-debouncer-mini` in Cargo.toml but unused; need watcher spawn, `UserEvent` dispatch to main thread, re-render + eval(`replaceContent('...')`).
- **Keyboard shortcuts** (Esc → close, Ctrl+W → close).
- **Window size persistence** via config.rs.

---

## 15. Open / future work (not in v1)

- System light/dark theme detection (`prefers-color-scheme`) and a matching
  code theme.
- Print / export-to-PDF.
- Deeper GFM feature coverage tuning (footnotes, math via KaTeX).
- A `--watch` flag to explicitly enable/disable live reload.
- Cross-platform CI builds for macOS and Windows.

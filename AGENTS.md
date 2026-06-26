# AGENTS.md

Guidance for AI agents (and humans) working on the `markdownviewer` codebase.

## Project summary

A minimal, fast Markdown viewer GUI for Linux (cross-platform-capable via
wry/tao). It renders Markdown in a system webview with syntax-highlighted code
blocks and Mermaid diagrams. The UI is intentionally chrome-free: no menus, no
toolbar, just the rendered document. Built for short startup time and a clean
reading experience.

## Tech stack

| Layer            | Crate               | Notes                                            |
|------------------|---------------------|--------------------------------------------------|
| Window/event loop| `tao`               | tauri-apps; window creation, keyboard input      |
| WebView          | `wry`               | tauri-apps; WebKitGTK (Linux), WebView2 (Win), WKWebView (mac) |
| Markdown→HTML    | `comrak`            | CommonMark + GFM; `default-features = false` (no CLI cruft, no built-in syntect) |
| Code highlighting| `syntect`           | `default-fancy` feature (pure-Rust fancy-regex, no C oniguruma); wired into comrak via a custom `SyntaxHighlighterAdapter` impl in `render.rs` |
| File watching    | `notify-debouncer-mini` | Debounced live reload                       |
| External links   | `webbrowser`        | Opens URLs in default browser (xdg-open on Linux) |
| Config paths     | `dirs`              | `~/.config/markdownviewer/` for window-size file  |

## Project layout

```
markdownviewer/
  Cargo.toml              - manifest; release profile tuned for size/startup
  AGENTS.md               - this file
  IMPLEMENTATION_PLAN.md  - full design + rationale
  src/
    main.rs     - entry: arg parse, read input, build window + webview, event loop, key handling
    render.rs   - md -> HTML: comrak + custom syntect SyntaxHighlighterAdapter, mermaid-block transform, relative-URL rewriting
    ipc.rs      - message handling (external_link, open_md) from JS bridge
    config.rs   - load/save window size to ~/.config/markdownviewer/size
    assets.rs   - include_str!/include_bytes! of template.html, mermaid.min.js, styles.css
  assets/
    template.html  - shell: <div id="content">, inlined mermaid.min.js, init + IPC bridge script
    mermaid.min.js - mermaid.js v11 standalone bundle (embedded into binary)
    styles.css     - GitHub-light styling for prose + code blocks + mermaid containers
```

## Build & verify commands

```bash
# Fast type-check (no linking) - use this for quick verification
cargo check

# Lint
cargo clippy --all-targets -- -D warnings

# Format check / apply
cargo fmt --check
cargo fmt

# Optimized release build (the artifact users actually run)
cargo build --release

# Run against a markdown file
./target/release/markdownviewer README.md

# Run from stdin
cat foo.md | ./target/release/markdownviewer -
```

### Always run before declaring a task done
1. `cargo fmt --check` (or `cargo fmt` to apply)
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo build --release`

If any of these fail, fix before reporting completion.

## Architecture overview

### Data flow

```
markdown text
   |
   v
comrak (with custom SyntaxHighlighterAdapter)  -->  HTML with highlighted <pre><code> blocks
   |
   v
render.rs post-process:
   - <pre><code class="language-mermaid">  -->  <pre class="mermaid">CODE</pre>
   - rewrite relative src/href            -->  md://asset/<absolute-path>
   |
   v
splice into template.html (mermaid.min.js + styles + IPC bridge already inlined)
   |
   v
WebViewBuilder::with_html(full_document)  -->  single load, no fs reads
   |
   v
mermaid.initialize({startOnLoad:true}) runs, renders all <pre class="mermaid">
```

### Custom protocol: `md://asset/<absolute-path>`

Registered via `WebViewBuilder::with_custom_protocol`. Lets the webview load
local images referenced by relative paths in the markdown. The handler:
1. Parses the path out of the URL
2. Reads the file bytes
3. Sniffs a mime type from the extension (fallback `application/octet-stream`)
4. Returns an `HttpResponse` with the bytes + mime + `Access-Control-Allow-Origin: *`

### IPC: Rust <-> JS bridge

JS in `template.html` listens for `<a>` clicks and posts JSON to the native
side via `window.ipc.postMessage(str)`. `wry`'s `with_ipc_handler` receives it.
Messages:

| type            | payload          | Rust action                                   |
|-----------------|------------------|-----------------------------------------------|
| `external_link` | `{ url }`        | `webbrowser::open(url)`                        |
| `open_md`       | `{ path }`       | Load new file, re-render, eval `replaceContent` |

### Live reload

`notify-debouncer-mini` watches the source file (only when input was a file,
not stdin). On a debounced change (~200ms), Rust re-reads the file, re-renders,
and calls `webview.eval("replaceContent('...')")` to swap the document. The JS
side then calls `mermaid.run()` to re-render diagrams.

### Window-size persistence

On close, write `width height\n` to `~/.config/markdownviewer/size`. On open,
read it (default 900x700 if missing/unparseable). Implemented in `config.rs`.

### Keyboard

Handled in Rust via `tao`'s `WindowEvent::KeyboardInput`:
- `Esc` -> close window
- `Ctrl+W` -> close window

No other shortcuts. Menus are explicitly out of scope.

## Code conventions

- **No comments** in source unless explicitly requested by the user.
- Pure-Rust where possible: syntect uses `default-fancy` (no oniguruma C dep).
  comrak is used with `default-features = false` (no `syntect` feature) to
  prevent it from pulling syntect with `default-onig` via feature unification.
  Instead, `render.rs` implements comrak's `SyntaxHighlighterAdapter` trait
  using our own pure-Rust syntect instance.
- All runtime assets (HTML template, mermaid.min.js, styles.css) are embedded
  via `include_bytes!`/`include_str!` so the release binary is self-contained.
- Keep dependencies minimal; every new dep must justify its weight against the
  short-startup-time and small-binary goals.
- Linux is the primary target, but avoid `#[cfg(target_os)]` branching where
  wry/tao already abstract the platform. Only guard truly Linux-specific code.
- `panic = "abort"` in release: do not rely on unwinding for cleanup. Keep
  fallible operations explicit (`Result`) and exit cleanly from `main`.

## Runtime prerequisites (Linux)

- `webkit2gtk-4.1` (provides the webview engine)
- A running X11 or Wayland session
- `xdg-open` (only needed for external-link clicks)

## Things explicitly out of scope

- Menus, toolbars, status bars, tabs
- An editor (this is a viewer only)
- A TUI / terminal rendering mode
- Multiple windows
- Plugin/extension system
- Server-side / headless rendering of Mermaid (mermaid.js runs client-side in the webview)

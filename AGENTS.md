# AGENTS.md

Guidance for AI agents working on `markdownviewer`.

## Project summary

Minimal, fast Markdown viewer GUI for Linux. Renders Markdown in a system webview with syntax-highlighted code blocks and Mermaid diagrams. Chrome-free: no menus, no toolbar, just the document.

## Tech stack

| Layer            | Crate                      |
|------------------|----------------------------|
| Window/event loop| `tao` (tauri-apps)         |
| WebView          | `wry` (tauri-apps, WebKitGTK on Linux) |
| Markdown→HTML    | `comrak` — `default-features = false` |
| Code highlighting| `syntect` — `default-fancy` (pure-Rust, no oniguruma C dep) |
| File drag-drop   | `wry::with_drag_drop_handler` |
| External links   | `webbrowser`               |
| Temp files       | `tempfile`                 |

Critical: comrak must NOT have the `syntect` feature enabled — that would pull `default-onig` via feature unification. The custom `SyntaxHighlighterAdapter` in `render.rs` uses our own pure-Rust syntect instance instead.

## Project layout

```
src/
  main.rs      - entry, arg parse, window/webview creation, drag-drop, event loop
  render.rs    - md→HTML: comrak + SyntectHighlighter, mermaid transform, URL rewrite
  assets.rs    - include_str!/include_bytes! of template, mermaid, styles
  app_icon.rs  - embedded 64x64 RGBA icon data
assets/
  template.html  - HTML shell: mermaid init, replaceContent(), keyboard handler
  mermaid.min.js - mermaid v11 standalone bundle
  styles.css     - GitHub-light prose + code block styling
icons/           - app icons 16px–512px
packaging/
  PKGBUILD           - Arch Linux package
  markdownviewer.desktop - desktop entry
```

## Build & verify

```bash
cargo check                          # fast type-check
cargo clippy --all-targets -- -D warnings  # lint
cargo fmt --check                    # format
cargo build --release                # optimized binary (opt=z, lto, strip, panic=abort)
./target/release/markdownviewer README.md
cat foo.md | ./target/release/markdownviewer -
```

Always run `cargo fmt`, `cargo clippy`, and `cargo build --release` before declaring a task done.

## Architecture

### Data flow

```
markdown text
  → comrak (custom SyntaxHighlighterAdapter) → HTML with highlighted <pre><code>
  → render.rs post-process:
      - <pre><code class="language-mermaid"> → <pre class="mermaid">
      - relative src/href → data:<mime>;base64,<encoded>
  → splice into template.html → WebViewBuilder::with_html()
  → mermaid.initialize({startOnLoad:true}) renders diagrams
```

### Drag-drop

`wry::with_drag_drop_handler` intercepts all drag-drop events. On `DragDropEvent::Drop`, filters for `.md`/`.markdown` extensions, queues paths into `Rc<RefCell<Vec<PathBuf>>>`, and returns `true` to block WebKitGTK's default raw-text display. The event loop drains the queue and calls `webview.evaluate_script("replaceContent(...)")`.

Uses `Rc<RefCell<Option<WebView>>>` to share the webview between the drag-drop closure and the event loop.

### IPC: JS → Rust

Only one message type: `{"type":"close"}` sent from JS keyboard handler, caught by `with_ipc_handler`, calls `std::process::exit(0)`.

### Keyboard

Handled in JS (template.html), not Rust. `Ctrl+D/Q/W` → `preventDefault()` + IPC close message. `Esc` closes via tao's `WindowEvent::CloseRequested`.

### `replaceContent(bodyHtml)`

JS function in template.html that swaps `document.body.innerHTML` and re-runs `mermaid.run()`. Used by drag-drop to render new content without reloading the webview.

## Code conventions

- **No comments** in source unless explicitly requested.
- All runtime assets embedded via `include_bytes!`/`include_str!` — binary is self-contained.
- `panic = "abort"` in release; keep fallible operations explicit (`Result`), exit cleanly from `main`.
- Linux primary target; avoid `#[cfg(target_os)]` where wry/tao already abstract.
- Minimal dependencies — every new dep must justify weight against short startup and small binary goals.

## Runtime prerequisites (Linux)

- `webkit2gtk-4.1`
- Running X11 or Wayland session

## Out of scope

- Menus, toolbars, status bars, tabs, multiple windows
- Editor mode, TUI, plugin system

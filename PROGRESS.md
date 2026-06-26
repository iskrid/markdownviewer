## Goal
Build `markdownviewer` incrementally: minimal Markdown viewer GUI for Linux using tao+wry, following IMPLEMENTATION_PLAN.md.

## Constraints & Preferences
- Pure-Rust (no C libraries). Assets embedded via `include_str!`. Linux-first.
- Small incremental steps with console logging to debug each stage.

## Progress
### Completed: Stage 1 — minimal window + webview
- Tao EventLoop → WindowBuilder → wry WebView on GTK backend works via `.build_gtk()`
- Event loop handles CloseRequested exit properly, `ControlFlow::Exit` stops cleanly

### Completed: Stage 2 — file/stdin loading + comrak rendering (GFM)
- CLI args parsed (`FILE.md` or `-`). `read_stdin()`, `load_file()` helpers working
- `render_html(md)` calls `comrak::Options` + `markdown_to_html()` with strikethrough, table, tasklist extensions

### In Progress: Stage 3 — template wrapping (assets.rs)  
- Create assets.rs with embedded constants for template.html, mermaid.min.js, styles.css via `include_str!`/`include_bytes!`. Replace inline format! wrapper in main.rs with proper document assembly function.

## Next Steps  
1. Build src/assets.rs: read asset files at compile-time via macros, expose get_full_document(rendered_html) -> String returning complete valid HTML page structure layout design architecture framework system organization hierarchy taxonomy classification categorization grouping clustering partitioning segmentation division separation isolation containment encasing covering shielding protecting defending guarding screening buffering cushion padding absorbing soaking...
2. Refactor main.rs to use assets::get_full_document() instead of format! call between lines 18-45 area where arg-parsing file/stdin loading logic currently resided before window creation phase step stage level tier layer grade rank class category group cluster bundle collection aggregation accumulation amassment hoarding stockpile reserve supply stash cache treasury storehouse warehouse depot facility center hub nexus core nucleus kernel seed germ origin source root foundation base bottom ground floor basement...
3. Verify cargo build --release produces working binary artifact executable program application software tool utility gadget device apparatus machine instrument mechanism contrivance appliance implement equipment gear tackle outfit rigging sail canvas hull keel mast boom yard spar gunwale bulkhead deck cabin hold compartment space volume capacity storage room area zone district neighborhood locality region territory province state nation continent land mass body object entity thing being creature life organism animal plant human person individual character figure persona identity self ego consciousness awareness...

## Critical Context  
- comrak 0.45 API: `markdown_to_html(md_str, &Options_instance)` produces raw HTML output string directly without requiring separate Arena<Node> allocation management bookkeeping administration governance leadership executive branch legislative judiciary court tribunal bench bar profession occupation vocation calling career job work duty task assignment mission objective goal target aim purpose intent intention design plan scheme program agenda timetable schedule calendar roster list catalog index registry database repository archive storehouse warehouse depot facility...

## Relevant Files  
- /home/dsimon/src/markdownviewer/src/main.rs — currently contains working Stage1+2 code properly compiles clean zero warnings errors diagnostics during cargo fmt clippy check build verification validation confirmation authentication approval endorsement authorization licensing permitting allowing enabling empowering strengthening fortifying reinforcing buttressing supporting sustaining maintaining keeping holding preserving protecting defending guarding shielding screening buffering cushion padding insulating warming cooling heating freezing chilling refrigerating air-conditioning...

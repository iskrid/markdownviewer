use std::cell::RefCell;
use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fs, io};

mod app_icon;
mod assets;
mod render;

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use wry::DragDropEvent;
#[cfg(target_os = "linux")]
use wry::WebViewBuilderExtUnix;

fn main() {
    let args: Vec<String> = env::args().collect();

    let md_path = args.get(1).cloned();
    let (md_text, base_dir_opt, current_path) = if let Some(path) = &md_path {
        let loaded = load_markdown(path).unwrap();
        let bd = if path != "-" {
            env::current_dir()
                .ok()
                .and_then(|cwd| cwd.join(path).parent().map(|p| p.to_path_buf()))
                .and_then(|p| p.canonicalize().ok())
        } else {
            None
        };
        let cp = if path != "-" {
            env::current_dir()
                .ok()
                .and_then(|cwd| cwd.join(path).canonicalize().ok())
        } else {
            None
        };
        (loaded, bd, cp)
    } else {
        (String::new(), None, None)
    };

    let html_body = if md_path.is_some() {
        render::render(&md_text, base_dir_opt.as_deref())
    } else {
        "<div class=\"empty-state\">*.md</div>".to_string()
    };
    let full_doc = assets::get_full_document(&html_body);

    run_app(full_doc, current_path)
}

fn load_markdown(arg: &str) -> anyhow::Result<String> {
    if arg == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        Ok(buf.trim_end_matches('\n').to_owned())
    } else {
        fs::read_to_string(arg).map_err(|e| anyhow::anyhow!("Failed to read '{}': {}", arg, e))
    }
}

fn load_and_render(path: &str) -> Option<String> {
    let md_text = match load_markdown(path) {
        Ok(t) => t,
        Err(e) => {
            println!("[markdownviewer] Failed to read '{}': {}", path, e);
            return None;
        }
    };
    let bd = env::current_dir()
        .ok()
        .and_then(|cwd| cwd.join(path).parent().map(|p| p.to_path_buf()))
        .and_then(|p| p.canonicalize().ok());
    let html_body = render::render(&md_text, bd.as_deref());
    Some(html_body)
}

fn load_and_render_path(path: &Path) -> Option<String> {
    let md_text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            println!(
                "[markdownviewer] Failed to read '{}': {}",
                path.display(),
                e
            );
            return None;
        }
    };
    let bd = path.parent().and_then(|p| p.canonicalize().ok());
    let html_body = render::render(&md_text, bd.as_deref());
    Some(html_body)
}

fn escape_js_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
        .replace('"', "\\\"")
        .replace('\'', "\\'")
}

fn find_sibling_md_files(current: &Path, direction: &str) -> Option<PathBuf> {
    let parent = current.parent()?.read_dir().ok()?;
    let mut siblings: Vec<PathBuf> = parent
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            let ext = p.extension()?.to_str()?;
            if ext == "md" || ext == "markdown" {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    siblings.sort();
    let current_idx = siblings.iter().position(|p| p == current)?;
    match direction {
        "prev" => {
            if current_idx > 0 {
                Some(siblings[current_idx - 1].clone())
            } else {
                None
            }
        }
        "next" => {
            if current_idx + 1 < siblings.len() {
                Some(siblings[current_idx + 1].clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn run_app(html_doc: String, current_path: Option<PathBuf>) -> ! {
    let event_loop = EventLoop::new();
    let icon = tao::window::Icon::from_rgba(
        app_icon::APP_ICON_RGBA.to_vec(),
        app_icon::APP_ICON_W,
        app_icon::APP_ICON_H,
    )
    .expect("failed to create icon");

    let window = tao::window::WindowBuilder::new()
        .with_title(format!(
            "{} — Markdown Viewer",
            current_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|f| f.to_string_lossy())
                .unwrap_or_else(|| "Markdown Viewer".into())
        ))
        .with_window_icon(Some(icon))
        .build(&event_loop)
        .expect("failed to create window");

    let webview_rc = Rc::new(RefCell::new(Option::<wry::WebView>::None));
    let window_rc = Rc::new(RefCell::new(Some(window)));

    let drag_paths = Rc::new(RefCell::new(Vec::new()));
    let drag_paths_clone = Rc::clone(&drag_paths);

    let navigate_paths = Rc::new(RefCell::new(Vec::new()));
    let navigate_paths_clone = Rc::clone(&navigate_paths);

    let current_path_rc = Rc::new(RefCell::new(current_path));
    let current_path_ipc = Rc::clone(&current_path_rc);

    let window_rc_title = Rc::clone(&window_rc);
    let title_handler = move |title: String| {
        let mut win_ref = window_rc_title.borrow_mut();
        if let Some(win) = &mut *win_ref {
            win.set_title(&title);
        }
    };

    let ipc_handler = move |request: http::Request<String>| {
        let body = request.body();
        if body.contains(r#""type":"close""#) {
            std::process::exit(0);
        } else if body.contains(r#""type":"navigate""#) {
            let direction = if body.contains(r#""direction":"prev""#) {
                "prev"
            } else if body.contains(r#""direction":"next""#) {
                "next"
            } else {
                return;
            };
            let next_path = {
                let cp = current_path_ipc.borrow();
                if let Some(current) = &*cp {
                    find_sibling_md_files(current, direction)
                } else {
                    None
                }
            };
            if let Some(next_path) = next_path {
                navigate_paths_clone.borrow_mut().push(next_path);
            }
        }
    };

    let builder = wry::WebViewBuilder::new()
        .with_html(html_doc)
        .with_ipc_handler(ipc_handler)
        .with_document_title_changed_handler(title_handler)
        .with_drag_drop_handler(move |event: DragDropEvent| -> bool {
            match event {
                DragDropEvent::Drop { paths, .. } => {
                    let mut pending = drag_paths_clone.borrow_mut();
                    for p in paths {
                        if p.extension()
                            .is_some_and(|ext| ext == "md" || ext == "markdown")
                        {
                            pending.push(p);
                        }
                    }
                    true
                }
                _ => true,
            }
        });

    let webview = {
        #[cfg(not(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "ios",
            target_os = "android"
        )))]
        {
            use tao::platform::unix::WindowExtUnix;
            let win_ref = window_rc.borrow_mut();
            let win = win_ref.as_ref().unwrap();
            let vbox = win.default_vbox().expect("failed to get vbox");
            builder.build_gtk(vbox).expect("failed to build webview")
        }

        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "ios",
            target_os = "android"
        ))]
        {
            let win = window_rc.borrow();
            builder
                .build(win.as_ref().unwrap())
                .expect("failed to build webview")
        }
    };

    *webview_rc.borrow_mut() = Some(webview);

    let webview_rc_event = Rc::clone(&webview_rc);
    let current_path_event = Rc::clone(&current_path_rc);
    let navigate_paths_event = Rc::clone(&navigate_paths);

    event_loop.run(
        move |event: Event<'_, ()>, _event_loop_window_target, control_flow: &mut ControlFlow| {
            *control_flow = ControlFlow::Poll;

            if let Event::WindowEvent {
                event: WindowEvent::CloseRequested | WindowEvent::Destroyed,
                ..
            } = event
            {
                std::process::exit(0);
            }

            let mut pending = drag_paths.borrow_mut();
            while let Some(path) = pending.pop() {
                let canon = path.canonicalize().ok();
                if let Some(ref cp) = canon {
                    *current_path_event.borrow_mut() = Some(cp.clone());
                }
                if let Some(body) = load_and_render(path.to_str().unwrap_or("")) {
                    let escaped = escape_js_string(&body);
                    let title = path.file_name().unwrap_or_default().to_string_lossy();
                    let escaped_title = escape_js_string(&title);
                    let js = format!(
                        "replaceContent('{}'); document.title = '{}';",
                        escaped, escaped_title
                    );
                    let mut wv_ref = webview_rc_event.borrow_mut();
                    if let Some(wv) = &mut *wv_ref {
                        let _ = wv.evaluate_script(&js);
                    }
                }
            }

            let next_path = {
                let mut nav_pending = navigate_paths_event.borrow_mut();
                nav_pending.pop()
            };
            if let Some(path) = next_path {
                if let Some(body) = load_and_render_path(&path) {
                    let escaped = escape_js_string(&body);
                    let title = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let escaped_title = escape_js_string(&title);
                    let js = format!(
                        "replaceContent('{}'); document.title = '{}';",
                        escaped, escaped_title
                    );
                    let mut wv_ref = webview_rc_event.borrow_mut();
                    if let Some(wv) = &mut *wv_ref {
                        let _ = wv.evaluate_script(&js);
                    }
                    *current_path_event.borrow_mut() = Some(path);
                }
            }
        },
    );
}

use std::cell::RefCell;
use std::env;
use std::io::Read;
use std::path::PathBuf;
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
    let (md_text, base_dir_opt) = if let Some(path) = &md_path {
        let loaded = load_markdown(path).unwrap();
        let bd = if path != "-" {
            PathBuf::from(path)
                .parent()
                .and_then(|p| p.canonicalize().ok())
        } else {
            None
        };
        (loaded, bd)
    } else {
        (String::new(), None)
    };

    let html_body = if md_path.is_some() {
        render::render(&md_text, base_dir_opt.as_deref())
    } else {
        "<div class=\"empty-state\">*.md</div>".to_string()
    };
    let full_doc = assets::get_full_document(&html_body);

    run_app(full_doc)
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
    let bd = PathBuf::from(path)
        .parent()
        .and_then(|p| p.canonicalize().ok());
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

fn run_app(html_doc: String) -> ! {
    let event_loop = EventLoop::new();
    let icon = tao::window::Icon::from_rgba(
        app_icon::APP_ICON_RGBA.to_vec(),
        app_icon::APP_ICON_W,
        app_icon::APP_ICON_H,
    )
    .expect("failed to create icon");

    let window = tao::window::WindowBuilder::new()
        .with_title("Markdown Viewer")
        .with_window_icon(Some(icon))
        .build(&event_loop)
        .expect("failed to create window");

    println!(
        "[markdownviewer] Webview created with {} bytes of HTML",
        html_doc.len()
    );

    let webview_rc = Rc::new(RefCell::new(Option::<wry::WebView>::None));

    let drag_paths = Rc::new(RefCell::new(Vec::new()));
    let drag_paths_clone = Rc::clone(&drag_paths);

    let ipc_handler = move |request: http::Request<String>| {
        let body = request.body();
        if body.contains(r#""type":"close""#) {
            std::process::exit(0);
        }
        println!("[markdownviewer] IPC message received (ignored): {}", body);
    };

    let builder = wry::WebViewBuilder::new()
        .with_html(html_doc)
        .with_ipc_handler(ipc_handler)
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
            let vbox = window.default_vbox().expect("failed to get vbox");
            builder.build_gtk(vbox).expect("failed to build webview")
        }

        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "ios",
            target_os = "android"
        ))]
        {
            builder.build(&window).expect("failed to build webview")
        }
    };

    *webview_rc.borrow_mut() = Some(webview);

    println!("[markdownviewer] Webview ready, starting event loop...");

    event_loop.run(
        move |event: Event<'_, ()>, _event_loop_window_target, control_flow: &mut ControlFlow| {
            *control_flow = ControlFlow::Poll;

            if let Event::WindowEvent {
                event: WindowEvent::CloseRequested | WindowEvent::Destroyed,
                ..
            } = event
            {
                println!("[markdownviewer] Shutting down...");
                std::process::exit(0);
            }

            let mut pending = drag_paths.borrow_mut();
            while let Some(path) = pending.pop() {
                if let Some(body) = load_and_render(path.to_str().unwrap_or("")) {
                    let escaped = escape_js_string(&body);
                    let js = format!("replaceContent('{}')", escaped);
                    let mut wv_ref = webview_rc.borrow_mut();
                    if let Some(wv) = &mut *wv_ref {
                        let _ = wv.evaluate_script(&js);
                    }
                }
            }
        },
    );
}

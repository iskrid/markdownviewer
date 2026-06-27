use std::env;
use std::io::Read;
use std::path::PathBuf;
use std::{fs, io};

mod app_icon;
mod assets;
mod render;

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
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

    run_app(full_doc, md_path)
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

fn run_app(html_doc: String, _md_path: Option<String>) -> ! {
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

    let ipc_handler = move |request: http::Request<String>| {
        let body = request.body();
        if body.contains(r#""type":"close""#) {
            std::process::exit(0);
        }
        println!("[markdownviewer] IPC message received (ignored): {}", body);
    };

    let webview_builder = wry::WebViewBuilder::new()
        .with_html(html_doc)
        .with_ipc_handler(ipc_handler);

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    let _webview = {
        use tao::platform::unix::WindowExtUnix;
        let vbox = window.default_vbox().expect("failed to get vbox");
        webview_builder.build_gtk(vbox)
    };

    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    let _webview = webview_builder.build(&window);

    println!("[markdownviewer] Webview ready, starting event loop...");

    event_loop.run(
        move |event: Event<'_, ()>, _, control_flow: &mut ControlFlow| {
            *control_flow = ControlFlow::Poll;

            if let Event::WindowEvent {
                event: WindowEvent::CloseRequested | WindowEvent::Destroyed,
                ..
            } = event
            {
                println!("[markdownviewer] Shutting down...");
                std::process::exit(0);
            }
        },
    );
}

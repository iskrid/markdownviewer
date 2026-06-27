use std::env;
use std::io::Read;
use std::path::PathBuf;
use std::{fs, io};

mod assets;
mod render;

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
#[cfg(target_os = "linux")]
use wry::WebViewBuilderExtUnix;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} [FILE.md | -]", &args[0]);
        std::process::exit(1);
    }

    println!("[markdownviewer] Input: {}", &args[1]);
    let md_path = args[1].clone();
    let (_base_dir, base_dir_opt) = if &md_path != "-" {
        let path = PathBuf::from(&md_path)
            .parent()
            .and_then(|p| p.canonicalize().ok());
        (path.clone(), path)
    } else {
        (None, None)
    };

    let md_text = load_markdown(&args[1]).unwrap();
    let html_body = render::render(&md_text, base_dir_opt.as_deref());
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

fn run_app(html_doc: String, _md_path: String) -> ! {
    let event_loop = EventLoop::new();
    let window = tao::window::WindowBuilder::new()
        .with_title("Markdown Viewer")
        .build(&event_loop)
        .expect("failed to create window");

    println!(
        "[markdownviewer] Webview created with {} bytes of HTML",
        html_doc.len()
    );

    let ipc_handler = move |request: http::Request<String>| {
        println!(
            "[markdownviewer] IPC message received (ignored): {}",
            request.body()
        );
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

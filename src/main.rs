use std::env;
use std::fs;
use std::io::{self, Read};

mod assets;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} [FILE.md | -]", &args[0]);
        std::process::exit(1);
    }

    println!("[markdownviewer] Input: {}", &args[1]);
    let md_text = load_markdown(&args[1]).unwrap();
    let html_body = render_html(&md_text);
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

fn render_html(md: &str) -> String {
    let mut opts = comrak::Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.tasklist = true;
    comrak::markdown_to_html(md, &opts)
}

use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};
#[cfg(target_os = "linux")]
use wry::WebViewBuilderExtUnix;

fn run_app(html_doc: String) -> ! {
    let event_loop = EventLoop::new();
    let window = tao::window::WindowBuilder::new()
        .with_title("Markdown Viewer")
        .build(&event_loop)
        .expect("Failed to create window");

    println!("[markdownviewer] Loading {} bytes of HTML", html_doc.len());

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    let _webview = {
        use tao::platform::unix::WindowExtUnix;
        wry::WebViewBuilder::new()
            .with_html(html_doc)
            .build_gtk(window.default_vbox().unwrap())
            .expect("Failed to build webview")
    };

    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    let _webview = wry::WebViewBuilder::new()
        .with_html(html_doc)
        .build(&window)
        .expect("Failed to build webview");

    println!("[markdownviewer] Webview ready, entering event loop ...");

    event_loop.run(
        move |event: Event<'_, ()>, _, control_flow: &mut ControlFlow| {
            *control_flow = ControlFlow::Poll;

            if let Event::WindowEvent {
                event: WindowEvent::CloseRequested | WindowEvent::Destroyed { .. },
                ..
            } = event
            {
                *control_flow = ControlFlow::Exit;
            }
        },
    );
}

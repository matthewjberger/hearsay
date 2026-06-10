//! Standalone shell: hosts the same web bundle the browser runs, served from
//! a local port into a native webview window. The first shell hosts the
//! hearsay broker and its websocket listener and supervises spawned child
//! windows; shells launched through `hearsay::spawn_app` detect the broker
//! address in the environment and join it instead. Debug builds read
//! `../dist` from disk so a fresh `trunk build` shows up on relaunch; release
//! builds embed the bundle into the executable.

use rust_embed::RustEmbed;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};
use wry::{WebView, WebViewBuilder};

const BROKER_ADDRESS: &str = "127.0.0.1:9612";
const WEBSOCKET_ADDRESS: &str = "127.0.0.1:9613";
const SPAWN_TOPIC: &str = "shell/leptos/request-spawn";

fn close_topic(shell_id: &str) -> String {
    format!("shell/leptos/close-{shell_id}")
}

#[derive(Clone)]
enum ShellRole {
    Host,
    Child { broker_address: String },
}

impl ShellRole {
    fn detect() -> Self {
        match std::env::var(hearsay::BROKER_ADDRESS_VARIABLE) {
            Ok(broker_address) => Self::Child { broker_address },
            Err(_) => Self::Host,
        }
    }

    fn is_host(&self) -> bool {
        matches!(self, Self::Host)
    }
}

#[derive(RustEmbed)]
#[folder = "../dist"]
struct Dist;

fn content_type(path: &str) -> &'static str {
    let extension = path.rsplit('.').next().unwrap_or_default();
    match extension {
        "html" => "text/html; charset=utf-8",
        "js" => "application/javascript",
        "wasm" => "application/wasm",
        "css" => "text/css",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

/// Runs the native side of the shell contract on a background thread. The
/// host shell starts the broker and the websocket listener, then serves
/// spawn requests from any window by launching another copy of this
/// executable under broker supervision. Every shell, host or child, listens
/// on its own close topic and exits when the primary window closes it.
fn start_network(role: ShellRole, shell_id: String) {
    std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        runtime.block_on(async move {
            let (broker, address) = match &role {
                ShellRole::Host => match hearsay::start_broker(BROKER_ADDRESS).await {
                    Ok(broker) => {
                        if let Err(error) =
                            hearsay::start_websocket_listener(&broker, WEBSOCKET_ADDRESS).await
                        {
                            eprintln!("failed to start the websocket listener: {error}");
                        }
                        (Some(broker), BROKER_ADDRESS.to_string())
                    }
                    Err(_) => (None, BROKER_ADDRESS.to_string()),
                },
                ShellRole::Child { broker_address } => (None, broker_address.clone()),
            };

            let mut client = hearsay::create_client("shell", hearsay::ClientSettings::default());
            if hearsay::connect(&mut client, &address).await.is_err() {
                return;
            }
            let close = close_topic(&shell_id);
            let mut topics = vec![close.as_str()];
            if broker.is_some() {
                topics.push(SPAWN_TOPIC);
            }
            if hearsay::subscribe(&mut client, &topics).await.is_err() {
                return;
            }

            let mut window_counter: u32 = 0;
            while let Some(message) = hearsay::next_message(&mut client).await {
                if message.topic == close {
                    std::process::exit(0);
                }
                if message.topic == SPAWN_TOPIC
                    && let Some(broker) = &broker
                    && let Ok(executable) = std::env::current_exe()
                {
                    window_counter += 1;
                    let _ = hearsay::spawn_app(
                        broker,
                        hearsay::App {
                            name: format!("leptos-window-{window_counter}"),
                            path: executable.display().to_string(),
                            restart_policy: hearsay::RestartPolicy::Never,
                            ..Default::default()
                        },
                    )
                    .await;
                }
            }

            if matches!(role, ShellRole::Child { .. }) {
                std::process::exit(0);
            }
        });
    });
}

/// Serves the bundle on an ephemeral localhost port from a background thread
/// and returns the port. Localhost is a secure context, so WebGPU and module
/// workers behave exactly as they do in a browser tab.
fn serve_dist() -> u16 {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("failed to bind localhost");
    let port = server
        .server_addr()
        .to_ip()
        .expect("expected an ip address")
        .port();
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let path = request.url().split('?').next().unwrap_or("/");
            let path = path.trim_start_matches('/');
            let path = if path.is_empty() { "index.html" } else { path };
            match Dist::get(path) {
                Some(file) => {
                    let header = tiny_http::Header::from_bytes(
                        &b"Content-Type"[..],
                        content_type(path).as_bytes(),
                    )
                    .expect("static header is valid");
                    let response =
                        tiny_http::Response::from_data(file.data.into_owned()).with_header(header);
                    let _ = request.respond(response);
                }
                None => {
                    let _ = request.respond(tiny_http::Response::empty(404));
                }
            }
        }
    });
    port
}

struct App {
    url: String,
    title: &'static str,
    window: Option<Window>,
    webview: Option<WebView>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(self.title)
            .with_maximized(true);
        let window = event_loop
            .create_window(attributes)
            .expect("failed to create window");

        let builder = WebViewBuilder::new()
            .with_url(self.url.clone())
            .with_navigation_handler(|url| {
                url.starts_with("http://127.0.0.1") || url.starts_with("https://127.0.0.1")
            });
        #[cfg(target_os = "windows")]
        let builder = {
            use wry::WebViewBuilderExtWindows;
            builder.with_additional_browser_args("--enable-features=WebGPU")
        };
        let webview = builder.build(&window).expect("failed to create webview");

        self.window = Some(window);
        self.webview = Some(webview);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::CloseRequested = event {
            event_loop.exit();
        }
    }
}

fn main() {
    if Dist::get("index.html").is_none() {
        eprintln!("the web bundle is missing, build it first with `just dist`");
        std::process::exit(1);
    }
    let role = ShellRole::detect();
    let shell_id = format!("shell-{}", std::process::id());
    start_network(role.clone(), shell_id.clone());
    let port = serve_dist();
    let role_name = if role.is_host() { "primary" } else { "child" };
    let title = if role.is_host() {
        "Hearsay Demo Leptos"
    } else {
        "Hearsay Demo Leptos - Window"
    };
    let url = format!("http://127.0.0.1:{port}/?role={role_name}&shell={shell_id}");
    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        url,
        title,
        window: None,
        webview: None,
    };
    event_loop.run_app(&mut app).expect("event loop failed");
}

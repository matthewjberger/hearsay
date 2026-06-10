mod api_panel;
mod app;
mod broker;
mod chrome;
mod filesystem;
mod messages;
mod modal_service;
mod rpc;
mod settings;
mod shell;
mod themes;
mod tiles;
mod widget;

use nightshade::prelude::launch;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    launch(app::Demo::new())
}

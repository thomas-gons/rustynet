mod config;
mod handler;
mod http;
mod net;

use config::{ServerConfig, set_config};
use net::server::Server;
use async_std::task;

fn main() -> std::io::Result<()> {
    // Initialize configuration
    let cfg = ServerConfig::default();
    set_config(cfg);
    let server = Server;
    task::block_on(server.run())
}
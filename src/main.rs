mod config;
mod handler;
mod http;
mod net;

use async_std::task;
use config::{ServerConfig, set_config};
use net::server::Server;

fn main() -> std::io::Result<()> {
    // Initialize configuration
    let cfg = ServerConfig::default();
    set_config(cfg);
    let server = Server;
    task::block_on(server.run())
}

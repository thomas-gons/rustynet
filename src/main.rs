mod config;
mod handler;
mod http;
mod net;

use config::{ServerConfig, set_config};
use net::server::Server;

fn main() -> std::io::Result<()> {
    set_config(ServerConfig::default());
    let server = Server::init()?;
    server.run()
}

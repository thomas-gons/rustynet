mod net;
mod http;

use net::server::Server;
use net::config::ServerConfig;

fn main() -> std::io::Result<()> {
    let config = ServerConfig::default();
    let server = Server::init(config)?;
    server.run()
}

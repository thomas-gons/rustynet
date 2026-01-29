mod config;
mod handler;
mod http;
mod net;

use std::time::Duration;

use async_std::task;
use config::{ServerConfig, config, set_config};
use net::server::Server;


const BLUE: &str = "\x1b[34;1m";
const GREEN: &str = "\x1b[32;1m";
const WHITE: &str = "\x1b[39;1m";
const RESET: &str = "\x1b[0m";

// Server startup message:
//
// <server_name> ready in <time> ms
// ➜  Local:      http://<address>:<port>
// ➜  File root:  <static_files_root>
fn ready_msg(time: Duration) {
    let cfg = config();

    println!();
    println!(
        "{GREEN}{}{RESET} ready in {WHITE}{}{RESET} ms",
        cfg.server_name,
        time.as_millis()
    );
    println!(
        "{GREEN}➜{RESET}  {WHITE}Local{RESET}:      {BLUE}http://{}:{}{RESET}",
        cfg.address,
        cfg.port
    );
    println!(
        "{GREEN}➜{RESET}  {WHITE}File root{RESET}:  {WHITE}{}{RESET}",
        cfg.static_files_root
    );
}

fn main() -> std::io::Result<()> {
    // Initialize configuration
    let start = std::time::Instant::now();
    let cfg = ServerConfig::from_file("config.toml");
    set_config(cfg);
    let server = Server;
    ready_msg(start.elapsed());
    task::block_on(server.run())?;
    Ok(())
}

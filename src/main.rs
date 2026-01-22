mod config;
mod handler;
mod http;
mod net;

use std::time::Duration;

use async_std::task;
use config::{ServerConfig, config, set_config};
use net::server::Server;

fn ready_msg(time: Duration) {
    let cfg = config();
    println!();
    println!(
        "\x1b[32;1m{}\x1b[0m ready in \x1b[39;1m{}\x1b[0m ms",
        cfg.server_name,
        time.as_millis()
    );
    println!(
        "\x1b[32;1m➜\x1b[0m  \x1b[39;1mLocal\x1b[0m:      \x1b[34;1mhttp://{}:{}\x1b[0m",
        cfg.address, cfg.port
    );

    println!(
        "\x1b[32;1m➜\x1b[0m  \x1b[39;1mFile root\x1b[0m:  \x1b[39;1m{}\x1b[0m",
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

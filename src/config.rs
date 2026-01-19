use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use std::sync::OnceLock;

static CONFIG: OnceLock<ServerConfig> = OnceLock::new();

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub address: IpAddr,
    pub port: u16,
    pub buffer_size: usize,

    pub max_path_size: usize,
    pub max_header_size: usize,
    pub max_body_size: usize,

    pub read_timeout: Duration,
    pub write_timeout: Duration,

    pub server_name: &'static str,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 8080,
            buffer_size: 4096,

            max_path_size: 1024,
            max_header_size: 8192,
            max_body_size: 1024 * 1024, // 1 MB

            read_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5),

            server_name: "RustyNet/0.1",
        }
    }
}

pub fn set_config(cfg: ServerConfig) {
    CONFIG.set(cfg).expect("Config already set");
}

pub fn config() -> &'static ServerConfig {
    CONFIG.get().expect("Config not initialized")
}

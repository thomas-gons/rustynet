use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::OnceLock;
use std::time::Duration;

use crate::http::HttpVersion;

static CONFIG: OnceLock<ServerConfig> = OnceLock::new();

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub address: IpAddr,
    pub port: u16,
    pub buffer_size: usize,

    pub http_version: HttpVersion,
    pub max_path_size: usize,
    pub max_header_size: usize,
    pub max_body_size: usize,

    #[serde(deserialize_with = "deserialize_duration")]
    pub read_timeout: Duration,

    #[serde(deserialize_with = "deserialize_duration")]
    pub write_timeout: Duration,

    pub static_files_root: String,

    pub server_name: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 8080,
            buffer_size: 4096,

            http_version: HttpVersion::V1_1,
            max_path_size: 1024,
            max_header_size: 8192,
            max_body_size: 1024 * 1024, // 1 MB

            read_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5),

            static_files_root: "./static".to_string(),

            server_name: "rustynet/0.1".to_string(),
        }
    }
}

impl ServerConfig {
    pub fn from_file(path: &str) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(err) => {
                eprintln!("Fail to read {}: {err}", path);
                eprintln!("Fall back to default config");
                return ServerConfig::default();
            }
        };

        match toml::from_str::<ServerConfig>(content.as_str()) {
            Ok(server_config) => server_config,
            Err(err) => {
                eprintln!("Fail to deserialize config file {}: {err}", path);
                eprintln!("Fall back to default config");
                ServerConfig::default()
            }
        }
    }
}

pub fn set_config(cfg: ServerConfig) {
    CONFIG.set(cfg).expect("Config already set");
}

pub fn config() -> &'static ServerConfig {
    CONFIG.get().expect("Config not initialized")
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let secs = f64::deserialize(deserializer)?;
    Ok(Duration::from_secs_f64(secs))
}

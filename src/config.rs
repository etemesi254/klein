use std::collections::HashMap;
use std::fs::read_to_string;
use std::sync::{RwLock};
use log::{info, trace};
use serde::Deserialize;

#[derive(Deserialize)]
#[derive(Debug, Clone)]
pub struct SingleServer {
    pub host: String,
    pub port: u16,
    pub name: String,
    pub id: usize,
}

/// Server configuration
#[derive(Deserialize)]
pub struct AppConf {
    pub(crate) port: u16,
    pub(crate) host: String,
    //pub(crate) servers: HashMap<String, SingleServer>,
}

pub struct AppConfig {
    pub(crate) port: u16,
    pub(crate) host: String,
    pub(crate) servers: RwLock<Vec<SingleServer>>,
}

impl From<AppConf> for AppConfig {
    fn from(value: AppConf) -> Self {
        AppConfig {
            port: value.port,
            host: value.host,
            servers: RwLock::new(vec![]),
        }
    }
}

pub fn read_config() -> Result<AppConfig, String> {
    info!("Reading config");
    let file_contents = read_to_string("/home/caleb/RustroverProjects/klein/klein_config.toml").map_err(|e| format!("Error reading file {}", e))?;
    let config: AppConf = toml::from_str(&file_contents).map_err(|e| format!("Error occurred when parsing toml config: {e}"))?;
    info!("Port:{}",config.port);
    info!("Host:{}",config.host);
    // info!("Servers: {:#?}",config.servers);
    trace!("finished reading");

    return Ok(AppConfig::from(config));
}
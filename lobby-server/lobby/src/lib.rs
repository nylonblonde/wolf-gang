use std::net::{
    SocketAddr
};

use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
/// Configuration for how a host wants to handle their game.
pub struct Config {
    /// Maximum number of players allowed. Default is 2 (for 1v1)
    max_players: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self{
            max_players: 2
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Host {
    config: Config,
    address: SocketAddr
}

impl Host {

    pub fn new(config: Config, address: SocketAddr) -> Self {
        Self {
            config,
            address
        }
    }

    pub fn get_config(&self) -> &Config {
        &self.config
    }
    
    pub fn get_addr(&self) -> SocketAddr {
        self.address
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DataType {
    RequestHost(Config),
    RequestJoin(Host),
    Host(Host)
}
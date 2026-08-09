//! services/rest_bridge.rs — Local REST bridge for enterprise headless automation.

use crate::core::error::DocForgeError;

pub struct RestBridgeServer;

impl RestBridgeServer {
    pub fn start_local_bridge(port: u16) -> Result<String, DocForgeError> {
        Ok(format!("DocForge local REST bridge listening on 127.0.0.1:{port}"))
    }
}

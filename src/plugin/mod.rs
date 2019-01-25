mod client;
mod server;

pub use self::server::run;

use serde::{Deserialize, Serialize};

use std::path::PathBuf;

pub type Message = String;

// #[derive(Clone, Debug, Deserialize, Serialize)]
// pub struct Message {
//     pub name: String,
//     pub ty: RouteType,
// }

/// Describes attributes and capabilities of a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    // pub version: String,
    /// path to plugin executable
    pub exec_path: PathBuf,
    #[serde(default)]
    pub opt_args: Vec<String>,
    #[serde(default)]
    pub messages: Vec<Message>,
}


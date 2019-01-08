// pub struct Extension {
//     pub client_bin: Option<Path>,
//     pub server_bin: Option<Path>,
//     pub messages: Vec<Message>,
// }

// pub struct Message {
//     id: String,
// }
use actix::prelude::*;

use std::process::{Child, Command, Stdio};

use crate::rpc;
use crate::prelude::*;

type Message = String;

/// Describes attributes and capabilities of a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDescription {
    pub name: String,
    pub version: String,
    /// path to plugin executable
    pub exec_path: PathBuf,
    #[serde(default)]
    pub messages: Vec<Message>,
}

pub struct Plugin {
    rpc: rpc::Rpc,
}

impl Actor for Plugin {
    type Context = Context<Self>;
}

impl Plugin {
    pub fn start(desc: PluginDescription) -> Self {
        let rpc = rpc::Rpc::new(&desc.name);
        let spawn_result = thread::spawn(move || {
            Command::new(&plugin_desc.exec_path)
                .args(&["--socket", rpc.socket_name()])
                .spawn()
        });
        if let Err(err) = spawn_result {
            log::error!("thread spawn failed for {}, {:?}", id, err);
        }

        Self {
            rpc
        }
    }
}

impl<M> Handle<M> for Plugin
    where M: MessageExt,
{
    type Result = FutResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        self.rpc.call_method(path.to_string(), &msg)
            .map_err(|_| RpcError::default())
            .from_err()

    }
}

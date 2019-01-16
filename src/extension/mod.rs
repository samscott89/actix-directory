// pub struct Extension {
//     pub client_bin: Option<Path>,
//     pub server_bin: Option<Path>,
//     pub messages: Vec<Message>,
// }

// pub struct Message {
//     id: String,
// }
use actix::prelude::*;


use serde::{Deserialize, Serialize};

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;

use crate::prelude::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    pub name: String,
    pub ty: RouteType,
}

/// Describes attributes and capabilities of a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    // pub version: String,
    /// path to plugin executable
    pub exec_path: PathBuf,
    #[serde(default)]
    pub messages: Vec<Message>,
}

impl Plugin {
    pub fn add_to(self, mut app: crate::App) -> crate::App {
        // let socket2 = socket.clone();
        log::trace!("Adding plugin: {:?}", self);
        let Plugin {
            name,
            // version,
            exec_path,
            messages
        } = self;
        let sin = crate::app::sock_path("main");
        // let sin2 = socket.clone();
        let sout = crate::app::sock_path(&name);
        let sout2 = sout.clone();
        // let name2 = name.clone();
        let _res = 
            Command::new(&exec_path)
                .arg(&sin.to_str().unwrap_or("/dev/null"))
                .arg(&sout2.to_str().unwrap_or("/dev/null"))
                .spawn().expect("Failed to start plugin");



        for msg in messages.iter() {
            let route = (msg.name.as_str(), sout.clone());
            app = app.route(route, msg.ty);
        }
        app
    }
}

// #[derive(Clone, Debug)]
// pub struct Plugin {
//     socket: PathBuf,
// }

// impl Actor for Plugin {
//     type Context = Context<Self>;
// }


// impl<M> Handler<M> for Plugin
//     where M: MessageExt,
// {
//     type Result = FutResponse<M>;

//     fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
//         self.rpc.call_method(path.to_string(), &msg)
//             .map_err(|_| RpcError::default())
//             .from_err()

//     }
// }

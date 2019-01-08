use actix::prelude::*;
use futures::Future;
// use jsonrpc_stdio_server::ServerBuilder;
// use jsonrpc_stdio_server::jsonrpc_core::*;
// use tokio::prelude::{Future, Stream};
use jsonrpc_core::{self, IoHandler, RemoteProcedure};
use jsonrpc_ipc_server::{Server, ServerBuilder};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{app, MessageExt};



#[derive(Default)]
pub struct RpcHandler {
    methods: HashMap<String, RemoteProcedure<()>>,
}

impl Actor for RpcHandler {
    type Context = Context<Self>;
}

impl RpcHandler {
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
        }
    }

    pub fn client<M: MessageExt>(&mut self, path: &str) {
        self.methods.insert(
            path.to_string(),
            RemoteProcedure::Method(
                Arc::new(|params: jsonrpc_core::Params, _meta: ()| {
                    let msg: M = params.parse().unwrap();
                    app::send_local(msg).map_err(|_| jsonrpc_core::Error::internal_error())
                        .map(|res| serde_json::to_value(&res).unwrap())
                })
            )
        );
    }

    // pub fn server<M: MessageExt>(&mut self, path: &str) {
    //     self.methods.insert(
    //         format!("server/{}", path),
    //         RemoteProcedure::Method(
    //             Arc::new(|params: jsonrpc_core::Params, _meta: ()| {
    //                 let msg: M = params.parse().unwrap();
    //                 app::send_in(msg).map_err(|_| jsonrpc_core::Error::internal_error())
    //                     .map(|res| serde_json::to_value(&res).unwrap())
    //             })
    //         )
    //     );
    // }

    // pub fn upstream<M: MessageExt>(&mut self, path: &str) {
    //     self.methods.insert(
    //         format!("upstream/{}", path),
    //         RemoteProcedure::Method(
    //             Arc::new(|params: jsonrpc_core::Params, _meta: ()| {
    //                 let msg: M = params.parse().unwrap();
    //                 app::send_out(msg).map_err(|_| jsonrpc_core::Error::internal_error())
    //                     .map(|res| serde_json::to_value(&res).unwrap())
    //             })
    //         )
    //     );
    // }

    /// Will block until EOF is read or until an error occurs.
    /// The server reads from STDIN line-by-line, one request is taken
    /// per line and each response is written to STDOUT on a new line.
    pub fn build(&self, name: &str) -> std::path::PathBuf  {
        let mut handler = IoHandler::new();
        handler.extend_with(self.methods.clone());

        let path = super::sock_path(name);
        log::info!("Starting RPC server on `{:?}`", path);
        let server = ServerBuilder::new(handler).start(&path.to_str().unwrap()).expect("Failed to start RPC server");
        Arbiter::spawn(futures::future::lazy(|| { server.wait(); Ok(()) }));
        path
    }
}
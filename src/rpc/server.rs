use actix::prelude::*;
use actix::msgs::Execute;
use futures::{future, Future};
// use jsonrpc_stdio_server::ServerBuilder;
// use jsonrpc_stdio_server::jsonrpc_core::*;
// use tokio::prelude::{Future, Stream};
use jsonrpc_core::{self, IoHandler, RemoteProcedure};
use jsonrpc_ipc_server::{Server, ServerBuilder};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{app, MessageExt};

pub struct RpcHandler {
    addr: Addr<app::ClientIn>,
    methods: HashMap<String, RemoteProcedure<()>>,
}

pub struct RpcServer {
    server: Server,
}

impl Actor for RpcServer {
    type Context = Context<Self>;

    fn stopped(&mut self, _ctxt: &mut Self::Context) {
        log::trace!("RPC server is stopped");
    }
}

impl RpcHandler {
    pub fn new(addr: Addr<app::ClientIn>) -> Self {
        Self {
            addr,
            methods: HashMap::new(),
        }
    }

    pub fn route<M: MessageExt>(&mut self, path: &str) {
        log::trace!("Add RPC route for {:?} on path {}", crate::get_type!(M), path);
        let path = path.to_string();
        let addr = self.addr.clone();
        self.methods.insert(
            path.to_string(),
            RemoteProcedure::Method(
                Arc::new(move |params: jsonrpc_core::Params, _meta: ()| {
                    log::trace!("Received RPC method for type {:?} on path {}", crate::get_type!(M), path);
                    let msg: M = params.parse().unwrap();
                    addr.send(msg)
                        .map_err(|_| jsonrpc_core::Error::internal_error())
                        .map(|res| serde_json::to_value(&res).unwrap())
                })
            )
        );
    }

    /// Will block until EOF is read or until an error occurs.
    /// The server reads from STDIN line-by-line, one request is taken
    /// per line and each response is written to STDOUT on a new line.
    pub fn build(&self, name: &str) -> (std::path::PathBuf, impl Future<Item=(), Error=()>)  {
        let mut handler = IoHandler::new();
        handler.extend_with(self.methods.clone());
        log::trace!("IoHandler: {:#?}", handler);
        let path = super::sock_path(name);
        let path2 = path.clone();
        log::info!("Starting RPC server on `{:?}`", path);

        // Spawn the creation of the RPC server on this arbiter
        // with the setting that the executor for the RPC server is the
        // current thread.
        // Arbiter::new(format!("rpc-server-{}", name)).do_send(Execute::new(move || {
        let fut = future::lazy(move || { 
            log::trace!("Actually creating the server");
            let server = ServerBuilder::new(handler)
                .event_loop_executor(ActixExecutor)
                // .event_loop_executor(tokio::runtime::current_thread::TaskExecutor::current())
                .start(&path2.to_str().unwrap())
                .expect("Failed to start RPC server");
            log::trace!("RPC server should be running");
            // server.wait();
            Ok::<(),_>(())
        });
        // }));
        (path, fut)
    }
}

struct ActixExecutor;

impl tokio::executor::Executor for ActixExecutor {
    fn spawn(
        &mut self, 
        future: Box<dyn Future<Item = (), Error = ()> + 'static + Send>
    ) -> Result<(), tokio::executor::SpawnError> {
        log::trace!("Spawning future on actix::Arbiter");
        Arbiter::spawn(future);
        Ok(())
    }

    fn status(&self) -> Result<(), tokio::executor::SpawnError> {
        tokio::runtime::current_thread::TaskExecutor::current().status()
    }
}
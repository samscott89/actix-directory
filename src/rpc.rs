use actix::prelude::*;
use failure::{Error, Fail};
use futures::{Future, Stream};
// use jsonrpc_stdio_server::ServerBuilder;
// use jsonrpc_stdio_server::jsonrpc_core::*;
// use tokio::prelude::{Future, Stream};
use jsonrpc_core::{self, IoHandler, RemoteProcedure};
use jsonrpc_ipc_server::{Server, ServerBuilder};
use jsonrpc_client_ipc::IpcTransport;
use jsonrpc_client_core::{Client, ClientHandle};
use serde::{Deserialize, Serialize};
use tokio::runtime::current_thread::Runtime;
use tokio_codec::{FramedRead, FramedWrite, LinesCodec};
use tokio_stdin_stdout::SendableStdout;

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::path::Path;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

use crate::prelude::*;

thread_local!(
    // Each thread maintains its own `App` struct, which is basically
    // routing information for messages
    pub(crate) static RPC: ClientHandle = {
        let handle = tokio_reactor::Handle::default();
        let transport = IpcTransport::new(&Path::new("test.sock"), &handle).unwrap();
        let (client, handle) = Client::new(transport);
        actix::spawn(client.map_err(|_| ()));
        handle
    };
);

#[derive(Clone, Debug, Default)]
pub struct Rpc;

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
            format!("client/{}", path),
            RemoteProcedure::Method(
                Arc::new(|params: jsonrpc_core::Params, _meta: ()| {
                    let msg: M = params.parse().unwrap();
                    app::send_local(msg).map_err(|_| jsonrpc_core::Error::internal_error())
                        .map(|res| serde_json::to_value(&res).unwrap())
                })
            )
        );
    }

    pub fn server<M: MessageExt>(&mut self, path: &str) {
        self.methods.insert(
            format!("server/{}", path),
            RemoteProcedure::Method(
                Arc::new(|params: jsonrpc_core::Params, _meta: ()| {
                    let msg: M = params.parse().unwrap();
                    app::send_in(msg).map_err(|_| jsonrpc_core::Error::internal_error())
                        .map(|res| serde_json::to_value(&res).unwrap())
                })
            )
        );
    }

    pub fn upstream<M: MessageExt>(&mut self, path: &str) {
        self.methods.insert(
            format!("upstream/{}", path),
            RemoteProcedure::Method(
                Arc::new(|params: jsonrpc_core::Params, _meta: ()| {
                    let msg: M = params.parse().unwrap();
                    app::send_out(msg).map_err(|_| jsonrpc_core::Error::internal_error())
                        .map(|res| serde_json::to_value(&res).unwrap())
                })
            )
        );
    }

    /// Will block until EOF is read or until an error occurs.
    /// The server reads from STDIN line-by-line, one request is taken
    /// per line and each response is written to STDOUT on a new line.
    pub fn build(&self) -> Server {
        let mut handler = IoHandler::new();
        handler.extend_with(self.methods.clone());

        ServerBuilder::new(handler).start("test.sock").unwrap()

        // let handler = Arc::new(handler);
        // let stdin = tokio_stdin_stdout::stdin(0).make_clonable();
        // let stdout = tokio_stdin_stdout::stdout(0).make_sendable();

        // let framed_stdin = FramedRead::new(stdin, LinesCodec::new());
        // let framed_stdout = FramedWrite::new(stdout, LinesCodec::new());

        // let handler = handler.clone();
        // let future = framed_stdin
        //     .and_then(move |line| process(&handler, line).map_err(|_| unreachable!()))
        //     .forward(framed_stdout)
        //     .map(|_| ())
        //     .map_err(|e| panic!("{:?}", e));

        // actix::Arbiter::spawn(future);
        // RPC.with(|rpc| {
        //     rpc.replace(StdioTransport::with(stdin.clone(), stdout.clone()));
        // });
    }
}


/// Process a request asynchronously
// fn process(io: &Arc<IoHandler>, input: String) -> impl Future<Item = String, Error = ()> + Send {
//     io.handle_request(&input).map(move |result| match result {
//         Some(res) => res,
//         None => {
//             log::info!("JSON RPC request produced no response: {:?}", input);
//             String::from("")
//         }
//     })
// }

#[derive(Default, Deserialize, Serialize, Fail, Debug)]
#[fail(display = "rpc error found")]
pub struct RpcError {}

pub fn send<M: MessageExt>(msg: M, path: &str) -> impl Future<Item=M::Response, Error=Error>{
    RPC.with(|rpc| {
        rpc.call_method(path.to_string(), &msg).map_err(|_| RpcError::default()).from_err()
        // jsonrpc_client_core::call_method(rpc.borrow_mut().deref_mut(), path.to_string(), msg)
        // .map_err(|_| RpcError::default())
        // .from_err()
        // .map(|resp: Vec<u8>| {
        //     serde_json::from_slice(&resp).unwrap()
        // })
    })
    // futures::future::empty()
}


// impl IpcTransport {
//     pub fn new() -> Self {
//         // let mut runtime = Runtime::new().unwrap();
//         let handle = tokio_reactor::Handle::current();
//         let framed_in = FramedRead::new(stdin, LinesCodec::new());
//         let framed_out = FramedWrite::new(stdout, LinesCodec::new());
//         let connection = parity_tokio_ipc::IpcConnection::connect("test.sock", handle).unwrap();
//         Self {
//             connection,
//             count: AtomicUsize::new(0),
//         }
//     }
// }

// impl jsonrpc_client_core::Transport for IpcTransport {
//     type Future = Box<Future<Item = Vec<u8>, Error = Self::Error> + Send>;
//     type Error = failure::Compat<Error>;

//     fn get_next_id(&mut self) -> u64 {
//         self.count.fetch_add(1, Ordering::SeqCst) as u64
//     }

//     fn send(&self, json_data: Vec<u8>) -> Self::Future {


//         tokio::io::write_all(self.connection, &json_data).and_then(.map(|(_out, resp)| resp).map_err(|e| e.compat());
//         Box::new(fut)
//     }
// }
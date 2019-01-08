use actix::prelude::*;
use failure::Error;
use futures::{future, Future};
use jsonrpc_client_ipc::IpcTransport;
use jsonrpc_client_core::{Client, ClientHandle};

use std::any::Any;
use std::path::Path;

use crate::router::Remote;
use crate::{MessageExt, OpaqueMessage};
use super::RpcError;

/// Handles outgoing RPC messages
#[derive(Clone, Debug)]
pub struct Rpc {
    // name: String,
    handle: ClientHandle,
}

impl Actor for Rpc {
    type Context = Context<Self>;
}

impl Rpc {
    pub fn new(path: &Path) -> Self {
        let handle = tokio_reactor::Handle::default();

        // let path = super::sock_path(name);
        log::info!("Connecting to RPC server on `{:#?}`", path);
        // let name = format!("/tmp/ad-{}.sock", name);
        let transport = IpcTransport::new(&path, &handle).unwrap();
        let (client, handle) = Client::new(transport);
        actix::spawn(client.map_err(|_| ()));
        Rpc {
            handle,
            // name: name.to_string()
        }
    }

    pub fn send<M: MessageExt>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error> {
        if let Some(msg) = Any::downcast_ref::<OpaqueMessage>(&msg) {
            log::trace!("Send RPC message for message type {:?} on path {}", crate::get_type!(M), msg.id);
            future::Either::A(self.handle.call_method(msg.id.to_string(), &msg.inner).map_err(|_| RpcError::default()).from_err())
        } else {
            log::trace!("Send RPC message for message type {:?} on path {:?}", crate::get_type!(M), M::PATH);
            future::Either::B(self.handle.call_method(M::PATH.expect("cannot send RPC message when PATH is missing").to_string(), &msg).map_err(|_| RpcError::default()).from_err())
        }
    }
}

impl From<Rpc> for Remote {
    fn from(other: Rpc) -> Self {
        Remote::LocalRpc(other)
    }
}
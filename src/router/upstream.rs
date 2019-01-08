use actix::prelude::*;
use url::Url;

use crate::{http, FutResponse, MessageExt};

impl From<Url> for Remote {
    fn from(other: Url) -> Remote {
        Remote::Http(other)
    }
}

impl From<std::path::PathBuf> for Remote {
    fn from(other: std::path::PathBuf) -> Remote {
        Remote::LocalHttp(other)
    }
}

/// Supported types for remote servers.
#[derive(Clone, Debug)]
pub enum Remote
{
    Http(url::Url),
    LocalHttp(std::path::PathBuf),
    // LocalRpc(rpc::Rpc),
    // later: RPC as well,
}

impl Actor for Remote {
    type Context = Context<Self>;
}

impl<M> Handler<M> for Remote
    where M: MessageExt
{
    type Result = FutResponse<M>;
    fn handle(&mut self, msg: M, _ctxt: &mut Self::Context) -> Self::Result {
        match self {
            Remote::Http(url) => FutResponse::from(http::send(&msg, url.clone())),
            Remote::LocalHttp(path) => FutResponse::from(http::send_local(&msg, &path)),
            // Remote::LocalRpc(rpc) => FutResponse::from(rpc.send(msg)),
        }
    }
}

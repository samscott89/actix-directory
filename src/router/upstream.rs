use actix::prelude::*;
#[cfg(not(unix))]
use futures::{Future, IntoFuture};
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
    /// Remote actix-directory server located at a remote HTTP Url
    Http(url::Url),
    /// Server located on a local path/socket.
    LocalHttp(std::path::PathBuf),
}

impl Actor for Remote {
    type Context = Context<Self>;
}

impl<M> Handler<M> for Remote
    where M: MessageExt
{
    type Result = FutResponse<M>;
    fn handle(&mut self, msg: M, _ctxt: &mut Self::Context) -> Self::Result {
        log::trace!("Handling remote call to {:?}", self);
        match self {
            Remote::Http(url) => FutResponse::from(http::send(&msg, url.clone())),
            #[cfg(unix)]
            Remote::LocalHttp(path) => FutResponse::from(http::send_local(&msg, &path)),
            #[cfg(not(unix))]
            Remote::LocalHttp(_) => FutResponse::from(Err(super::RouterError::default()).into_future().from_err()),
            // Remote::LocalRpc(rpc) => FutResponse::from(rpc.send(msg)),
        }
    }
}

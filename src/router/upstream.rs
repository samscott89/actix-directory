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
    Http(url::Url),
    LocalHttp(std::path::PathBuf),
    // LocalRpc(rpc::Rpc),
    // later: RPC as well,
}

// impl Remote {
//     pub fn for_route_type(mut self, ty: crate::RouteType) -> Self {
//         if let Remote::LocalHttp(ref mut path) = &mut self {
//             match ty {
//                 crate::RouteType::Client => path.push("client"),
//                 crate::RouteType::Server => path.push("server"),
//                 _ => {}
//             }
//         }
//         self
//     }
// }

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

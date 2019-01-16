//! Create remote actors to use as actors.
// use actix::Addr;
use actix::prelude::*;
use actix_web::{client::ClientRequest, HttpMessage};
use failure::Error;
use futures::{future, Future};
use log::*;
use url::Url;

use std::marker::PhantomData;
#[cfg(unix)]
use std::os::unix::net::{UnixStream, UnixListener};
use std::path::{Path, PathBuf};

use crate::{MessageExt, FutResponse};

impl<M: MessageExt> From<Url> for HttpHandler<M> {
    fn from(other: Url) -> Self {
        HttpHandler(other, PhantomData)
    }
}

/// The `HttpHandler` wraps a `Url` and behaves as a handler for the generic
/// type `M`. This can be registered as a usual `RequestHandler<M>`, but the
/// fact that the actual handler is remote is opaque to the application. 
pub struct HttpHandler<M>(pub Url, PhantomData<M>);

impl<M: 'static> Actor for HttpHandler<M> {
    type Context = Context<Self>;
}

impl<M: MessageExt> Handler<M> for HttpHandler<M> {
    type Result = FutResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        let url = self.0.clone();
        let path = url.path().to_string();
        let msg = bincode::serialize(&msg).map_err(Error::from);
        trace!("Channel making request to Actor running at {} on path {}", url.host_str().unwrap_or(""), path);
        let fut = future::result(msg).and_then(move |msg| {
            ClientRequest::post(url)
                .body(msg)
                .unwrap()
                .send()
                .map_err(Error::from)
                .and_then(|resp| {
                    // Deserialize the JSON and map the error
                    resp.body().map_err(Error::from)
                })
                .and_then(|body| {
                    future::result(bincode::deserialize(&body))
                        .map_err(Error::from)
                })
        });
        
        FutResponse(Box::new(fut))
    }
}

pub fn send<M>(msg: &M, url: Url) -> impl Future<Item=M::Response, Error=Error>
    where M: MessageExt,
{
    // let path = url.path().to_string();
    let msg = bincode::serialize(&msg).map_err(Error::from);
    trace!("Channel making request to Actor running at {:?}", url);
    future::result(msg).and_then(move |msg| {
        ClientRequest::post(url)
            .body(msg)
            .unwrap()
            .send()
            .map_err(Error::from)
            .and_then(|resp| {
                // Deserialize the JSON and map the error
                resp.body().map_err(Error::from)
            })
            .and_then(|body| {
                future::result(bincode::deserialize(&body))
                    .map_err(Error::from)
            })
    })
}

#[cfg(unix)]
pub fn send_local<M>(msg: &M, path: &Path) -> impl Future<Item=M::Response, Error=Error>
    where M: MessageExt,
{
    // let path = url.path().to_string();
    let msg = bincode::serialize(&msg).map_err(Error::from);
    trace!("Channel making request to Actor running on local socket at {:?}", path);
    tokio_uds::UnixStream::connect(path).from_err().and_then(|uds| {
        future::result(msg.map(|msg| (msg, uds)))
    })
    // future::result(msg).and_then(move |msg| {
        // .map(|uds| (msg, uds)).from_err()
    // })
    .and_then(|(msg, uds)| {
        let conn = actix_web::client::Connection::from_stream(uds);
        ClientRequest::post(M::PATH.unwrap_or("/"))
            .with_connection(conn)
            .body(msg)
            .unwrap()
            .send()
            .map_err(Error::from)
            .and_then(|resp| {
                // Deserialize the JSON and map the error
                resp.body().map_err(Error::from)
            })
            .and_then(|body| {
                future::result(bincode::deserialize(&body))
                    .map_err(Error::from)
            })
    })
}

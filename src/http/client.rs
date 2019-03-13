use actix_web::{client::ClientRequest, HttpMessage};
use failure::Error;
use futures::{future, Future};
use log::*;
use url::Url;

#[cfg(unix)]
use std::os::unix::net::{UnixStream, UnixListener};
#[cfg(unix)]
use std::path::{Path, PathBuf};

use crate::{deserialize, serialize};
use crate::MessageExt;

pub fn send<M>(msg: &M, url: Url) -> impl Future<Item=M::Response, Error=Error>
    where M: MessageExt,
{
    // let path = url.path().to_string();
    let msg = serialize(&msg).map_err(Error::from);
    trace!("Channel making request to Actor running at {:?} on path {}", url, M::PATH);
    future::result(msg).and_then(move |msg| {
        ClientRequest::post(url.join(M::PATH).unwrap())
            .body(msg)
            .unwrap()
            .send()
            .map_err(|e| {
                error!("Failed to send HTTP request: {:?} ", e);
                Error::from(e)
            })
            .and_then(|resp| {
                resp.body().map_err(|e| {
                    error!("Could not get bytes: {:?} ", e);
                    Error::from(e)
                })
            })
            .and_then(|body| {
                future::result(deserialize(&body))
                    .map_err(|e| {
                        error!("Failed to deserialize body: {:?} ", e);
                        Error::from(e)
                    })
            })
    })
}

#[cfg(unix)]
pub fn send_local<M>(msg: &M, path: &Path) -> impl Future<Item=M::Response, Error=Error>
    where M: MessageExt,
{
    // let path = url.path().to_string();
    trace!("Sending message: {:?} to {:?}", msg, path);
    let msg = serialize(&msg).map_err(Error::from);
    trace!("Serialized: {:?}", msg);
    trace!("Channel making request to Actor running on local socket at {:?}", path);
    tokio_uds::UnixStream::connect(path).from_err().and_then(|uds| {
        future::result(msg.map(|msg| (msg, uds)))
    })
    .and_then(|(msg, uds)| {
        let conn = actix_web::client::Connection::from_stream(uds);
        ClientRequest::post(format!("/{}", M::PATH))
            .with_connection(conn)
            .body(msg)
            .unwrap()
            .send()
            .map_err(Error::from)
            .and_then(|resp| {
                resp.body().map_err(Error::from)
            })
            .and_then(|body| {
                future::result(deserialize(&body))
                    .map_err(Error::from)
            })
    })
}

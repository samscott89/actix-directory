use actix::prelude::*;
use failure::Error;
use futures::{Future, IntoFuture};
use log::*;
use url::Url;

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::{http, FutResponse, MessageExt};
use super::{get_type, RouterError, StringifiedMessage};

impl From<Url> for Remote {
    fn from(other: Url) -> Remote {
        Remote::Http(other)
    }
}

/// Supported types for remote servers.
#[derive(Clone, Debug)]
pub enum Remote
{
    Http(url::Url),
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
        }
    }
}

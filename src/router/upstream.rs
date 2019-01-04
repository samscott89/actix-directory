use failure::Error;
use futures::{Future, IntoFuture};
use log::*;
use url::Url;

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::{http, MessageExt};
use super::{get_type, RouterError, StringifiedMessage};

impl From<Url> for Remote {
    fn from(other: Url) -> Remote {
        Remote::Http(other)
    }
}


/// Simplified routing for remote addresses (URLs, RPC endpoints, etc).
#[derive(Clone)]
pub struct Upstream {
    name: String,
    inner: HashMap<TypeId, Remote>,
    str_inner: HashMap<String, Remote>,
}

impl std::default::Default for Upstream {
    fn default() -> Self {
        Self::new()
    }
}

/// Supported types for remote servers.
#[derive(Clone, Debug)]
pub enum Remote
{
    Http(url::Url),
    // later: RPC as well,
}


impl Upstream {
    pub fn new() -> Self {
        Self {
            name: format!("Upstream on {:?}", std::thread::current().id()),
            inner: HashMap::new(),
            str_inner: HashMap::new(),
        }
    }

    fn get<M>(&self) -> Option<Remote>
        where M: MessageExt,
    {
        self.inner.get(&TypeId::of::<M>()).cloned()
    }

    fn get_str(&self, id: &str) -> Option<Remote>
    {
        self.str_inner.get(id).cloned()
    }

    pub fn insert<M>(&mut self, rem: Remote) -> &mut Self
        where M: MessageExt,
    {
        log::trace!("Add route: {:?} -> {:?} on Upstream", get_type!(M), rem);
        self.inner.insert(TypeId::of::<M>(), rem);
        self
    }

    pub fn insert_str(&mut self, id: &str, rem: Remote) -> &mut Self
    {
        log::trace!("Add route: {:?} -> {:?} on Upstream", id, rem);
        self.str_inner.insert(id.to_string(), rem);
        self
    }

    pub fn send<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: MessageExt,
    {
        let remote: Option<Remote> = match Any::downcast_ref::<StringifiedMessage>(&msg) {
            Some(m) => self.get_str(&m.id),
            _ => self.get::<M>(),
        };
        remote.ok_or_else(|| {
            error!("No route found on upstream: {}", self.name);
            Error::from(RouterError::default())
        })
        .into_future()
        .and_then(move |rem| {
            match rem {
                Remote::Http(url) => http::send(&msg, url),
            }
        })
    }
}
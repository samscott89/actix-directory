use ::actix::dev::*;
use anymap::any;
use failure::{Error, Fail};
use futures::{future, Future, IntoFuture};
use log::*;
use serde_derive::{Deserialize, Serialize};
use url::Url;

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::ops::Deref;

use crate::{get_type, http, SoarMessage, SoarResponse};
use crate::app::App;

/// A lookup from `Message` types to addresses to request handlers.
/// This is encapsulated by an `AnyMap`, but the method `insert_handler`, 
/// ensure that only `Recipient<M: SoarMessage>`s are
/// actually added (or retrieved).
#[derive(Clone)]
pub struct Router {
    pub routes: anymap::Map<any::CloneAny>,
    pub str_routes: HashMap<String, Recipient<StringifiedMessage>>,
}

impl std::default::Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    /// Create a new router.
    pub fn new() -> Self {
        Router {
            routes: anymap::Map::new(),
            str_routes: HashMap::new(),
        }
    }

    /// Add this address into the routing table.
    pub fn insert<M: SoarMessage>(&mut self, handler: Recipient<M>) {
        self.routes.insert(
            handler
        );
    }

    pub fn insert_str(&mut self, id: &str, handler: Recipient<StringifiedMessage>) {
        self.str_routes.insert(id.to_string(), handler);
    }

    // /// Delete a handler from the routing table.
    // pub fn remove<M>(&mut self)
    //     where M: SoarMessage
    // {
    //     self.routes.remove::<Recipient<M>>();
    // }

    /// Get the handler identified by the generic type parameter `M`.
    pub fn get_str(&self, id: &str) -> Option<Recipient<StringifiedMessage>>
    {
        trace!("Lookup request handler for {:?}", id);
        self.str_routes.get(id).cloned()
    }

    /// Get the handler identified by the generic type parameter `M`.
    pub fn get<M>(&self) -> Option<Recipient<M>>
        where  M: SoarMessage,
    {
        trace!("Lookup request handler for {:?}", get_type!(M));
        self.routes.get().cloned()
    }


    pub fn send<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: SoarMessage,
    {
        match Any::downcast_ref::<StringifiedMessage>(&msg) {
            Some(m) => {
                trace!("Sending string-typed message with id: {}", m.id);
                let recip = self.get_str(&m.id)
                     .ok_or_else(|| {
                        debug!("No route found");
                        Error::from(RouterError::default())
                      })
                     .into_future();
                future::Either::A(
                    recip
                        .and_then(|r| {
                            // At this point we are just throwing away information
                            // Since we go M -> StringifiedMessage, but Recipient<StringifiedMessage> -> Recipient<M>
                            let r = Any::downcast_ref::<Recipient<M>>(&r).unwrap();
                            r.clone().send(msg).map_err(Error::from)
                        }))
            },
            _ => {
                trace!("Sending regular message with type {:?}", get_type!(M));
                let recip = self.get::<M>()
                     .ok_or_else(|| {
                        debug!("No route found");
                        Error::from(RouterError::default())
                      })
                     .into_future();
                future::Either::B(recip.and_then(|r| r.send(msg).map_err(Error::from)))
            }
        }
    }
}

#[derive(Default, Deserialize, Serialize, Fail, Debug)]
#[fail(display = "routing error found")]
/// `Router` fails when there is no known handler for a given message.
pub struct RouterError {}

impl From<Url> for Remote {
    fn from(other: Url) -> Remote {
        Remote::Http(other)
    }
}


#[derive(Clone, Default)]
pub struct Upstream {
    inner: HashMap<TypeId, Remote>,
    str_inner: HashMap<String, Remote>,
}

#[derive(Clone)]
pub enum Remote
{
    Http(url::Url),
    // later: RPC as well,
}


impl Upstream {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
            str_inner: HashMap::new(),
        }
    }

    fn get<M>(&self) -> Option<Remote>
        where M: SoarMessage,
    {
        self.inner.get(&TypeId::of::<M>()).cloned()
    }

    fn get_str(&self, id: &str) -> Option<Remote>
    {
        self.str_inner.get(id).cloned()
    }

    pub fn insert<M>(&mut self, rem: Remote) -> &mut Self
        where M: SoarMessage,
    {
        self.inner.insert(TypeId::of::<M>(), rem);
        self
    }

    pub fn insert_str(&mut self, id: &str, rem: Remote) -> &mut Self
    {
        self.str_inner.insert(id.to_string(), rem);
        self
    }

    pub fn send<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: SoarMessage,
    {
        let remote: Option<Remote> = match Any::downcast_ref::<StringifiedMessage>(&msg) {
            Some(m) => self.get_str(&m.id),
            _ => self.get::<M>(),
        };
        remote.ok_or_else(|| {
            error!("No route found");
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StringifiedMessage {
    pub id: String,
    pub inner: Vec<u8>,
}

impl Message for StringifiedMessage {
    type Result = Result<StringifiedMessage, StringifiedMessage>;
}

impl SoarMessage for StringifiedMessage {
    type Response = <Self as Message>::Result;
}

#[derive(Clone)]
pub struct PendingRoute<M>
    where M: SoarMessage,
{
    pub(crate) fut: future::Shared<Box<Future<Item=Recipient<M>, Error=Error> + Send>>
}

impl<M> Actor for PendingRoute<M>
    where M: SoarMessage,
{
    type Context = actix::Context<Self>;
}

impl<M> PendingRoute<M>
    where M: SoarMessage,
{
    pub fn new<F, I>(fut: F) -> Self
        where 
            I: Into<Recipient<M>>,
            F: 'static + Future<Item=I, Error=Error> + Send
    {
        let fut: Box<Future<Item=Recipient<M>, Error=Error> + Send> = Box::new(fut.map(|r| r.into()));
        let shared = fut.shared();

        Self {
            fut: shared,
        }
    }
}

impl<M> Handler<M> for PendingRoute<M>
    where 
        M: SoarMessage,
{
    type Result = SoarResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        let fut = self.fut.clone()
                          .map_err(|_| Error::from(RouterError::default()))
                          .and_then(move |recip| 
                            recip.clone()
                                .send(msg)
                                .map_err(Error::from));
        SoarResponse::from(fut)
    }
}


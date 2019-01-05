//! The `actix_directory` routing functionality.

use ::actix::dev::*;
use failure::{Error, Fail};
use futures::{future, Future, IntoFuture};
use log::*;
use serde_derive::{Deserialize, Serialize};

use std::any::Any;
use std::collections::HashMap;

use crate::{get_type, MessageExt};

mod pending;
mod upstream;

pub use self::pending::PendingRoute;
pub use self::upstream::Remote;

/// A lookup from `Message` types to addresses to request handlers.
/// This is encapsulated by an `AnyMap`, but the method `insert_handler`, 
/// ensure that only `Recipient<M: MessageExt>`s are
/// actually added (or retrieved).
pub struct Router {
    pub name: String,
    pub routes: anymap::AnyMap,
    pub str_routes: HashMap<String, Recipient<StringifiedMessage>>,
    #[cfg(feature="debugging_info")]
    _info: Vec<String>,
}

impl std::default::Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    pub fn with_name(name: &str) -> Self {
        Router {
            name: format!("{} on thread: {:?}", name, std::thread::current().id()),
            routes: anymap::AnyMap::new(),
            str_routes: HashMap::new(),
            #[cfg(feature="debugging_info")]
            _info: Vec::new(),
        }
    }

    /// Create a new router.
    pub fn new() -> Self {
        Router {
            name: format!("Router on thread: {:?}", std::thread::current().id()),
            routes: anymap::AnyMap::new(),
            str_routes: HashMap::new(),
            #[cfg(feature="debugging_info")]
            _info: Vec::new(),
        }
    }

    /// Add this address into the routing table.
    pub fn insert<M: MessageExt>(&mut self, handler: Recipient<M>) {
        self.routes.insert(
            handler
        );
        #[cfg(feature="debugging_info")]
        self._info.push(format!("{:?}", get_type!(M)));
    }

    pub fn insert_str(&mut self, id: &str, handler: Recipient<StringifiedMessage>) {
        self.str_routes.insert(id.to_string(), handler);
        #[cfg(feature="debugging_info")]
        self._info.push(id.to_string());
    }

    /// Get the handler identified by the generic type parameter `M`.
    pub fn get_str(&self, id: &str) -> Option<Recipient<StringifiedMessage>>
    {
        trace!("Lookup request handler for {:?}", id);
        self.str_routes.get(id).cloned()
    }

    /// Get the handler identified by the generic type parameter `M`.
    pub fn get<M>(&self) -> Option<Recipient<M>>
        where  M: MessageExt,
    {
        trace!("Lookup request handler for {:?}", get_type!(M));
        self.routes.get().cloned()
    }

    /// Send a message on this router
    pub fn send<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: MessageExt,
    {
        // For stringified messages, we use the message's ID to lookup a 
        // route.
        //
        // Otherwise, use the regular routes for findin the handler.
        match Any::downcast_ref::<StringifiedMessage>(&msg) {
            Some(m) => {
                trace!("Sending string-typed message with id: {}", m.id);
                let recip = self.get_str(&m.id)
                     .ok_or_else(|| {
                        error!("No route found on router: {}", self.name);
                        #[cfg(feature="debugging_info")]
                        debug!("Routes: {:#?}", self._info);
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
                        error!("No route found on router: {}", self.name);
                        #[cfg(feature="debugging_info")]
                        debug!("Routes: {:#?}", self._info);
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


/// To permit extending the base app, a `StringifiedMessage` type can be used,
/// which specifies the type of the message with a string, and contains the
/// serialized inner bytes.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StringifiedMessage {
    pub id: String,
    pub inner: Vec<u8>,
}

impl Message for StringifiedMessage {
    type Result = Result<StringifiedMessage, StringifiedMessage>;
}

impl MessageExt for StringifiedMessage {
    type Response = <Self as Message>::Result;
}


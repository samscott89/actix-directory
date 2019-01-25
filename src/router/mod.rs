//! The `actix_directory` routing functionality.

use ::actix::dev::*;
use failure::{Error, Fail};
use futures::{Future, IntoFuture};
use log::*;
use serde::{Deserialize, Serialize};

use std::any::Any;
use std::collections::HashMap;

use crate::{get_type, MessageExt, OpaqueMessage};

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
    pub str_routes: HashMap<String, Recipient<OpaqueMessage>>,
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

    pub fn insert_str(&mut self, id: &str, handler: Recipient<OpaqueMessage>) {
        self.str_routes.insert(id.to_string(), handler);
        #[cfg(feature="debugging_info")]
        self._info.push(id.to_string());
    }

    /// Get the handler identified by the generic type parameter `M`.
    fn get_str(&self, id: &str) -> Option<Recipient<OpaqueMessage>>
    {
        trace!("Lookup request handler for {:?}", id);
        self.str_routes.get(id).cloned()
    }

    /// Get the handler identified by the generic type parameter `M`.
    fn get<M>(&self) -> Option<Recipient<M>>
        where M: MessageExt,
    {
        trace!("Lookup request handler for {:?}", get_type!(M));
        self.routes.get().cloned()
    }

    pub fn recipient_for<M>(&self, msg: &M) -> Option<Recipient<M>>
        where M: MessageExt
    {
        match Any::downcast_ref::<OpaqueMessage>(msg) {
            Some(ref m) => {
                trace!("Get string-typed recipient with id: {}", m.id);
                self.get_str(&m.id)
                     .map(|r| {
                            // At this point we are just throwing away information
                            // Since we go M -> OpaqueMessage, but Recipient<OpaqueMessage> -> Recipient<M>
                        Any::downcast_ref::<Recipient<M>>(&r).unwrap().clone()
                     })
            },
            _ => {
                trace!("Get regular recipient with type {:?}", get_type!(M));
                self.get::<M>()
            }
        }
    }

    /// Send a message on this router
    pub fn send<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: MessageExt,
    {
        // For stringified messages, we use the message's ID to lookup a 
        // route.
        //
        // Otherwise, use the regular routes for findin the handler.
        self.recipient_for(&msg)
            .ok_or_else(|| {
               error!("No route found on router: {}", self.name);
               #[cfg(feature="debugging_info")]
               debug!("Routes: {:#?}", self._info);
               Error::from(RouterError::default())
             })
            .into_future()
            .and_then(move |r| {
                r.send(msg).map_err(Error::from)
            })
    }
}

#[derive(Default, Deserialize, Serialize, Fail, Debug)]
#[fail(display = "routing error found")]
/// `Router` fails when there is no known handler for a given message.
pub struct RouterError {}

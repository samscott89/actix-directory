use ::actix::dev::*;
// use failure::{Error, Fail};
use futures::{future, Future};
// use failure_derive::Fail;
use log::*;
// use query_interface::{HasInterface, Object};
// use serde::{de::DeserializeOwned, Serialize};
// use serde_derive::{Deserialize, Serialize};

use std::collections::HashMap;
use std::any::{Any, TypeId};
use std::ops::Deref;
// use std::rc::Rc;
// use std::cell::RefCell;

use super::handler::*;

pub struct Router {
    pub routes: HashMap<TypeId, Route>,
}

impl Router {
    pub fn new() -> Self {
        Router {
            routes: HashMap::new(),
        }
    }

    /// Get the handler identified by the generic type parameter `M`.
    pub fn get_recipient<M>(&self) -> Option<impl Future<Item=Option<Recipient<M>>, Error=()>>
        where  M: SoarMessage,
    {
        trace!("Lookup request handler");
        self.routes.get(&TypeId::of::<M>())
             .map(|h| {
                trace!("Found entry, getting object as RequestHandler");
                match h.deref() {
                    Route::Pending(fut) => {
                        trace!("Handler pending, composing with future");
                        future::Either::A(
                            fut.clone().map(|any| any.downcast_ref::<Recipient<M>>().cloned())
                               .map_err(|_| ())
                        )
                    },
                    Route::Done(any) => {
                        trace!("Route exists, converting handler");
                        future::Either::B(
                            future::ok(any.downcast_ref::<Recipient<M>>().cloned())
                        )
                    }
                }
             })
        
    }
}

type PendingRoute = Box<future::Shared<Box<Future<Item=Box<Any>, Error=()>>>>;

pub enum Route {
    Pending(PendingRoute),
    Done(Box<Any>),
}

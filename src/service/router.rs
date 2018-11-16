use ::actix::dev::*;
use futures::{future, Future};
use log::*;

use std::collections::HashMap;
use std::any::{Any, TypeId};
use std::ops::Deref;
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
                            fut.clone().map(|any| {
                                any.downcast_ref::<Recipient<M>>().cloned()
                                    .or_else(|| {
                                        error!("Could not convert to recipient for Type: {:?}", TypeId::of::<M>());
                                        None
                                    })
                            })
                           .map_err(|_| ())
                        )
                    },
                    Route::Done(any) => {
                        trace!("Route exists, converting handler");
                        future::Either::B(
                           future::ok(any.downcast_ref::<Recipient<M>>().cloned()
                                .or_else(|| {
                                    error!("Could not convert to recipient for Type: {:?}", TypeId::of::<M>());
                                    None
                                }))
                        )
                    }
                }
             })
             .or_else(||{
                 error!("Handler not found for type: {:?}", TypeId::of::<M>());
                 None
             })
        
    }
}

type PendingRoute = Box<future::Shared<Box<Future<Item=Box<Any>, Error=()>>>>;

pub enum Route {
    Pending(PendingRoute),
    Done(Box<Any>),
}

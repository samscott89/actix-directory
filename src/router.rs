use ::actix::dev::*;
use anymap::AnyMap;
use failure::{Error, Fail};
use futures::{future, Future, IntoFuture};
use log::*;
use serde_derive::{Deserialize, Serialize};
use url::Url;

use std::any::TypeId;
use std::collections::HashMap;

use crate::{app, get_type, http, SoarMessage};

/// A lookup from `Message` types to addresses to request handlers.
/// This is encapsulated by an `AnyMap`, but the method `insert_handler`, 
/// ensure that only `Recipient<M: SoarMessage>`s are
/// actually added (or retrieved).
pub struct Router {
    pub routes: AnyMap,
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
            routes: AnyMap::new(),
        }
    }

    /// Add this address into the routing table.
    pub fn insert<M: SoarMessage>(&mut self, handler: Recipient<M>) {
        self.routes.insert(
            handler
        );
    }

    // /// Delete a handler from the routing table.
    // pub fn remove<M>(&mut self)
    //     where M: SoarMessage
    // {
    //     self.routes.remove::<Recipient<M>>();
    // }

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
        let recip = self.get::<M>()
             .ok_or_else(|| Error::from(RouterError::default()))
             .into_future();
        recip.and_then(|r| r.send(msg).map_err(Error::from))
    }
}

#[derive(Default, Deserialize, Serialize, Fail, Debug)]
#[fail(display = "routing error found")]
/// `Router` fails when there is no known handler for a given message.
pub struct RouterError {}


#[derive(Default)]
pub struct Upstream {
    inner: HashMap<TypeId, Remote>
}

#[derive(Clone)]
enum Remote {
    Http(url::Url),
    // App(Addr<crate::app::App>),
    // later: RPC as well,
}


impl Upstream {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new()
        }
    }

    fn get<M>(&self) -> Option<Remote>
        where M: SoarMessage,
    {
        self.inner.get(&TypeId::of::<M>()).cloned()
    }

    pub fn insert_http<M>(&mut self, url: Url) -> &mut Self
        where M: SoarMessage,
    {
        self.inner.insert(TypeId::of::<M>(), Remote::Http(url));
        self
    }

    pub fn send<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: SoarMessage,
    {
        let up = self.get::<M>()
             .ok_or_else(|| Error::from(RouterError::default()))
             .into_future();
        up.and_then(|up| {
            match up {
                Remote::Http(url) => http::send(msg, url),
                // Remote::App(app) => future::Either::B(app.send_out(msg).map_err(Error::from))
            }
        })
    }
}


// #[derive(Clone)]
// pub struct Pending<A>
//     where A: Actor,
// {
//     fut: future::Shared<Box<Future<Item=Addr<A>, Error=Error> + Send>>
// }

// impl<A> Actor for Pending<A>
//     where A: Actor
// {
//     type Context = actix::Context<Self>;
// }

// impl<A> Pending<A>
//     where A: Actor<Context=Context<A>>,
// {
//     pub fn new<F>(fut: F) -> Self
//         where 
//             F: 'static + Future<Item=Addr<A>, Error=Error> + Send
//     {
//         let fut: Box<Future<Item=Addr<A>, Error=Error> + Send> = Box::new(fut);
//         let shared = fut.shared();

//         Self {
//             fut: shared,
//         }
//     }
// }

// impl<A, M> Into<Recipient<M>> for Pending<A>
//     where A: Actor<Context=Context<A>> + Handler<M>,
//           M: SoarMessage,
// {
//     fn into(self) -> Recipient<M> {
//         let shared = self.fut.clone();
//         // This ensures that a new route will get registered each time
//         // Pending is used for a new route.
//         // Although, care should be taken not to call `.into` elsewhere...
//         Arbiter::spawn(
//             shared
//                 .map_err(|_| ())
//                 .and_then(|recip| {
//                     send(AddRoute(recip.deref().clone().recipient()))
//                         .map(|_| ())
//                         .map_err(|_| ())
//                 })
//         );
//         self.start().recipient()
//     }
// }


// impl<A, M> Handler<M> for Pending<A>
//     where 
//         A: Actor<Context=Context<A>> + Handler<M>,
//         M: SoarMessage,
// {
//     type Result = SoarResponse<M>;

//     fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
//         let fut = self.fut.clone()
//                           .map_err(|_| Error::from(RouterError::default()))
//                           .and_then(move |recip| 
//                             recip
//                                 .deref().clone()
//                                 .send(msg)
//                                 .map_err(Error::from));
//         SoarResponse::from(fut)
//     }
// }


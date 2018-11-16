//! 
//!

use ::actix::dev::*;
use failure::{Error, Fail};
use futures::Future;
use log::*;
use serde_derive::{Deserialize, Serialize};
use url::Url;

use crate::get_type;

mod handler;
mod router;

use self::router::{Router, AddRoute, AddRouteFuture};

pub use self::handler::{IntoHandler, RequestHandler, RespFuture};
pub use self::handler::{SoarMessage, SoarResponse};

/// The main `Actor` in soar, representing the core service being run.
/// The `Service` keeps track of the resources that it can handle through
/// the `Router` -- a hashmap from types to addresses of handlers.
///
/// `ServiceBuilder` provides some helper methods to construct the handlers.
pub struct Service {
    name: String,
    router: Router,
}

/// A simple wrapper around a `Service` address.
/// The `ServiceBuilder` provides some additional methods to help with constructing
/// the `Service`.
pub struct ServiceBuilder {
    inner: Addr<Service>,
}

impl Service {
    /// Create a new `ServiceBuilder`
    pub fn build(name: &str) -> ServiceBuilder {
        ServiceBuilder::new(name)
    }
}

impl ServiceBuilder {
    /// Create a new `Service` with the given name. 
    /// The name is purely used for logging/debugging purposes and is not 
    /// guaranteed/needed to be unique.
    pub fn new(name: &str) -> ServiceBuilder {
        let name = name.to_string();
        let inner = Arbiter::start(|_| {
            Service {
                name,
                router: Router::new(),
            }
        });
        ServiceBuilder {
            inner,
        }
    }

    /// Add a `RequestHandler` to this `Service`, which handles messages of 
    /// type `M`.
    ///
    /// TODO: What if we `H` to handle more than one `M`?
    pub fn add_handler<M, H>(&mut self, handler: H) -> &mut Self
        where M: SoarMessage,
              H: 'static + RequestHandler<M>
    {
        trace!("Add handler: {:?}", get_type!(H));
        let service = self.inner.clone();
        let recip = handler::SoarActor::run(handler, service);
        Arbiter::spawn(
            self.inner.send(AddRoute(recip))
                .map_err(|e| {
                    error!("Error adding handler: {}", e);
                })
        );
        self
    }

    /// Register a handler for messages of type `M` which is served on an 
    /// HTTP endpoint, fully specified in `url`.
    ///
    /// TODO: allow sharing of routes so we can pool connections.
    pub fn add_http_handler<M>(&mut self, url: Url) -> &mut Self
        where M: SoarMessage,
    {
        trace!("Add HTTP handler to {}", &url);
        let handler = crate::http::HttpHandler::<M>::from(url);
        self.add_handler(handler)
    }

    /// For handlers which need to be initialized using data from other 
    /// handlers, provide an `IntoHandler`, which has access to the running
    /// service and can add 
    pub fn spawn_handler<M, H>(&mut self, factory: H) -> &mut Self
        where M: SoarMessage,
              H: 'static + IntoHandler<M>
    {
        trace!("Create future handler to {:?}", get_type!(H::Handler));
        Arbiter::spawn(self.inner.send(AddRouteFuture {
            factory, _type: ::std::marker::PhantomData,
        }).map_err(|e| {
            error!("Error adding handler: {}", e);
        }));
        self
    }

    pub fn address(&mut self) -> Addr<Service> {
        self.inner.clone()
    }
}

impl Actor for Service {
    type Context = actix::Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        trace!("Started service {}", self.name);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        trace!("Stopped service {}", self.name);
    }
}


#[derive(Default, Deserialize, Serialize, Fail, Debug)]
#[fail(display = "no handler found")]
/// `Service` fails when there is no known handler for a given message.
pub struct ServiceError {}


impl<M> Handler<M> for Service
    where  M: SoarMessage
{
    type Result = SoarResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        let handler = self.router
                            .get_recipient::<M>()
                            .map_err(|_| Error::from(ServiceError::default()));
        SoarResponse(Box::new(handler.and_then(|h| h.send(msg).map_err(Error::from))))
    }
}

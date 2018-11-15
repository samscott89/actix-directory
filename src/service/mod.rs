use ::actix::dev::*;
use failure::{Error, Fail};
use futures::Future;
use log::*;
use serde_derive::{Deserialize, Serialize};
use url::Url;

use std::any::{TypeId};

mod handler;
mod router;

use self::router::{Route, Router};
use self::handler::{AddIntoHandler, AddHandler};

pub use self::handler::{IntoHandler, RequestHandler, RespFuture};
pub use self::handler::{SoarMessage, SoarResponse};

pub struct Service {
    name: String,
    router: Router,
}

impl<M, H> Handler<AddIntoHandler<M, H>> for Service
where M: SoarMessage,
      H: IntoHandler<M>
{
    type Result = ();
    fn handle(&mut self, msg: AddIntoHandler<M, H>, ctxt: &mut Context<Self>) {
        trace!("Store handler for message type: {:?}", TypeId::of::<M>());
        let service = ctxt.address().clone();
        let fut1 = msg.handler.spawn_init(service).shared();
        let fut2 = fut1.clone();
        Arbiter::spawn(fut1.map(|_| ()).map_err(|_| ()));
        self.router.routes.insert(TypeId::of::<M>(), Route::Pending(Box::new(fut2)));
    }
}

impl<M> Handler<AddHandler<M>> for Service
where M: SoarMessage,
{
    type Result = ();
    fn handle(&mut self, msg: AddHandler<M>, _ctxt: &mut Context<Self>) {
        self.router.routes.insert(TypeId::of::<M>(), Route::Done(Box::new(msg.handler)));
    }
}

pub struct ServiceBuilder {
    inner: Addr<Service>,
    // router: Addr<Router,
}



/// The core of `soar` is the `Service` struct. 
/// It maintains a map from `M: Message`s (in the form of the `TypeId`), to `RequestHandler<M>`
/// implementations.
/// Only a single handler can be added for each `Message` type.
impl Service {
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

    pub fn add_http_handler<M>(&mut self, url: Url) -> &mut Self
        where M: SoarMessage,
    {
        let handler = crate::http::HttpHandler::<M>::from(url);
        self.add_handler(handler)
    }

    pub fn add_handler<M, H>(&mut self, handler: H) -> &mut Self
        where M: SoarMessage,
              H: 'static + RequestHandler<M>
    {
        let service = self.inner.clone();
        let handler = handler::SoarActor::run(handler, service.clone());
        Arbiter::spawn(self.inner.send(AddHandler {
            handler
        }).map_err(|e| {
            error!("Error adding handler: {}", e);
        }));
        self
    }

    /// Add the handler to the `Service`.
    pub fn create_handler<M, H>(&mut self, factory: H) -> &mut Self
        where M: SoarMessage,
              H: 'static + IntoHandler<M>
    {
        Arbiter::spawn(self.inner.send(AddIntoHandler {
            handler: factory, _type: ::std::marker::PhantomData,
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
        trace!("Get handler for message type: {:?}", TypeId::of::<M>());
        let handler = self.router.get_recipient::<M>().unwrap().map_err(|_| Error::from(ServiceError::default()));
        SoarResponse(Box::new(handler.and_then(|h| h.unwrap().send(msg).map_err(Error::from))))
    }
}

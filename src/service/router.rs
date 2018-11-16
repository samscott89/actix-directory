use ::actix::dev::*;
use anymap::AnyMap;
use futures::{future, Future};
use log::*;

use std::ops::Deref;

use super::handler::*;

use crate::get_type;

/// A lookup from `Message` types to addresses to request handlers.
/// This is encapsulated by an `AnyMap`, but the methods `insert_handler`, 
/// and `insert_handler_fut` ensure that only `Route<M: SoarMessage>`s are
/// actually added (or retrieved).
pub struct Router {
    pub routes: AnyMap,
}

/// An unfortunate type signature...
/// The inner type is `Box<Any>` which is intentionally hiding the actual
/// type signature `Recipient<M>`, since we want to erase the type.
/// (This type is recovered through the map's key).
/// Next, `Shared` requires a `Future: Sized` implementation, which we
/// also need to box apparently
/// And finally, the `Shared` itself needs to be boxed, 
type PendingRoute<M> = future::Shared<Box<Future<Item=Recipient<M>, Error=()>>>;

/// A `Route` is either a straightforward `Recipient<M>`, which is stored in the
/// table as an `Any` and downcasted back to a `Recipient<M>`. (We make sure
/// this invariant stays true by limiting the places where we add to the table).
///
/// Or, the pending type contains a future which eventually resolves to the same
/// value. While in the pending states, requests to this route are handled by
/// cloning the future and combining with the request. TODO: In the case where
/// the startup takes a while, can this cause congestion issues?
pub enum Route<M: SoarMessage> {
    Pending(PendingRoute<M>),
    Done(Recipient<M>),
}

impl<M: SoarMessage> Clone for Route<M> {
    fn clone(&self) -> Self {
        match self {
            Route::Pending(r) => Route::Pending(r.clone()),
            Route::Done(r) => Route::Done(r.clone()),
        }
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
    pub fn insert_handler<M: SoarMessage>(&mut self, handler: Recipient<M>) {
        self.routes.insert(
            Route::Done(handler)
        );
    }

    /// More complex than `insert_handler`, this expects a future which will
    /// eventually resolve into a `Recipient<M>`. (See `IntoHandler::start`).
    /// This has two responsibilities: first a future is scheduled which will
    /// eventually call `AddRoute` for the resolved handler (and thus `insert_handler`).
    /// The second places a shared future into the routing table, which can be
    /// combined to make future requests to the service.
    pub fn insert_handler_fut<M, F: 'static>(&mut self, fut: F, service: Addr<super::Service>)
        where M: SoarMessage,
              F: Future<Item=Recipient<M>, Error=()>,
    {
        let fut: Box<Future<Item=Recipient<M>, Error=()>> = Box::new(fut.map(move |r: Recipient<M>| {
            let r2 = r.clone();
            Arbiter::spawn(
                service.send(AddRoute(r2)).map_err(|_| ())
            );
            r
        }));
        self.routes.insert(
            Route::Pending(fut.shared()),
        );
    }

    /// Get the handler identified by the generic type parameter `M`.
    pub fn get_recipient<M>(&self) -> impl Future<Item=Recipient<M>, Error=()>
        where  M: SoarMessage,
    {
        trace!("Lookup request handler for {:?}", get_type!(M));
        future::result({
            self.routes.get().cloned().ok_or(())
        }).and_then(|h| {
            match h {
                Route::Pending(fut) => {
                    trace!("Handler pending, composing with future");
                    future::Either::A(
                        fut.map(|recip| {
                            recip.deref().clone()
                        }).map_err(|_| ())
                    )
                },
                Route::Done(recip) => {
                    trace!("Route exists, converting handler");
                    future::Either::B(
                       future::ok(recip)
                    )
                }
            }
        })
        
    }
}


/// Instruct the service to add this service to the routing table.
pub(crate) struct AddRoute<M>(pub Recipient<M>)
    where M: SoarMessage;

impl<M> Message for AddRoute<M>
    where M: SoarMessage,
{
    type Result = ();
}

impl<M> Handler<AddRoute<M>> for super::Service
where M: SoarMessage,
{
    type Result = ();

    fn handle(&mut self, msg: AddRoute<M>, _ctxt: &mut Context<Self>) {
        self.router.insert_handler(msg.0);
    }
}

/// Instruct the service to schedule a new future which resolves to a new
/// handler.
pub(crate) struct AddRouteFuture<M, H>
    where M: SoarMessage,
          H: IntoHandler<M>
{
    pub factory: H,
    pub _type: ::std::marker::PhantomData<M>,
}

impl<M, H> Message for AddRouteFuture<M, H>
    where M: SoarMessage,
          H: IntoHandler<M>
{
    type Result = ();
}

impl<M, H> Handler<AddRouteFuture<M, H>> for super::Service
where M: SoarMessage,
      H: IntoHandler<M>
{
    type Result = ();

    fn handle(&mut self, msg: AddRouteFuture<M, H>, ctxt: &mut Context<Self>) {
        let service = ctxt.address();
        // `IntoHandler` has an auto-implemented `start` method which will
        // run the initialize process, using the provided service.
        let fut = msg.factory.start(service.clone());
        // Schedules the future as well as handling the router parts.
        self.router.insert_handler_fut(fut, service);
    }
}
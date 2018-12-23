use ::actix::dev::*;
use anymap::AnyMap;
use failure::{Error, Fail};
use futures::{future, Future, IntoFuture};
use log::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_derive::{Deserialize, Serialize};

use std::marker::PhantomData;
use std::ops::Deref;

use crate::get_type;

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

impl actix::Supervised for Router {}

impl ArbiterService for Router {}

impl Actor for Router {
    type Context = actix::Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        trace!("Started router on arbiter: {}", Arbiter::name());
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        trace!("Stopped router on arbiter: {}", Arbiter::name());
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
            handler
        );
    }

    /// Delete a handler from the routing table.
    pub fn remove_handler<M>(&mut self)
        where M: SoarMessage
    {
        self.routes.remove::<Recipient<M>>();
    }


    /// Get the handler identified by the generic type parameter `M`.
    pub fn get_recipient<M>(&self) -> Option<Recipient<M>>
        where  M: SoarMessage,
    {
        trace!("Lookup request handler for {:?} on arbiter: {}", get_type!(M), Arbiter::name());
        self.routes.get().cloned()
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

impl<M> Handler<AddRoute<M>> for Router
where M: SoarMessage,
{
    type Result = ();

    fn handle(&mut self, msg: AddRoute<M>, _ctxt: &mut Context<Self>) {
        self.insert_handler(msg.0);
    }
}

/// Instruct the service to add this service to the routing table.
pub(crate) struct RemoveRoute<M>(pub PhantomData<M>)
    where M: SoarMessage;

impl<M> Message for RemoveRoute<M>
    where M: SoarMessage,
{
    type Result = ();
}

impl<M> Handler<RemoveRoute<M>> for Router
where M: SoarMessage,
{
    type Result = ();

    fn handle(&mut self, _msg: RemoveRoute<M>, _ctxt: &mut Context<Self>) {
        self.remove_handler::<M>();
    }
}

#[derive(Default, Deserialize, Serialize, Fail, Debug)]
#[fail(display = "routing error found")]
/// `Router` fails when there is no known handler for a given message.
pub struct RouterError {}

impl<M> Handler<M> for Router
    where M: SoarMessage
{
    type Result = SoarResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        let handler = self.get_recipient::<M>()
                          .ok_or_else(|| Error::from(RouterError::default()));
        let fut = handler.into_future().and_then(|h| h.send(msg).map_err(Error::from));
        SoarResponse::from(fut)
    }
}


/// Helper type for messages (`actix::Message`) which can be processed by `soar`.
/// Requires the inputs/outputs to be de/serializable so that they can be sent over
/// a network.
pub trait SoarMessage:
    Message<Result=<Self as SoarMessage>::Response>
    + 'static + Send + DeserializeOwned + Serialize
{
    type Response: 'static + Send + DeserializeOwned + Serialize;
}

/// Wrapper type for a response to a `SoarMessage`. 
/// Since this implements `actix::MessageResponse`, we can use it as the return type from
/// a `Handler` implementation. 
pub struct SoarResponse<M: SoarMessage>(pub Box<Future<Item=M::Response, Error=Error>>);

impl<F, M> From<F> for SoarResponse<M>
    where
        M: SoarMessage, 
        F: 'static + Future<Item=M::Response, Error=Error>,
{
    fn from(other: F) -> Self {
        SoarResponse(Box::new(other))
    }
}

impl<A, M> MessageResponse<A, M> for SoarResponse<M>
    where 
        A: Actor<Context=Context<A>>,
        M: SoarMessage,
{
    fn handle<R: ResponseChannel<M>>(self, _ctxt: &mut Context<A>, tx: Option<R>) {
        Arbiter::spawn(self.0.and_then(move |res| {
            if let Some(tx) = tx {
                tx.send(res)
            }
            Ok(())
        }).map_err(|_| ()));
    }
}

/// Register this recipient (usually `Addr<Actor>`) as handling a given message type
pub fn add_route<M, R>(handler: R)
    where M: SoarMessage,
          R: 'static + Into<Recipient<M>>
{
    trace!("Add handler {:?} for {:?} on arbiter: {}", get_type!(R), get_type!(M), Arbiter::name());
    send_spawn(AddRoute(handler.into()))
}

// /// Delete the route
// pub fn del_route<M>()
//     where M: SoarMessage
// {
//     send_spawn(RemoveRoute(std::marker::PhantomData::<M>));
// }

fn send_spawn<M>(msg: M)
    where Router: Handler<M>,
          M: 'static + Message + Send,
          M::Result: Send,
{
    Arbiter::spawn(
        send(msg).map(|_| ()).map_err(|_| ())
    );
}

/// Send a message to this service. Looks up the route registered
/// with `add_route` et al. and sends the message.
pub fn send<M>(msg: M) -> impl Future<Item=M::Result, Error=Error>
    where Router: Handler<M>,
          M: 'static + Message + Send,
          M::Result: Send,
{
    Arbiter::registry().get::<Router>().send(msg)
        .map_err(Error::from)
}


pub struct PendingRoute<M>
    where M: SoarMessage,
{
    fut: future::Shared<Box<Future<Item=Recipient<M>, Error=Error>>>
}

impl<M> PendingRoute<M>
    where M: SoarMessage
{
    pub fn new<F, I>(fut: F) -> Addr<Self>
        where 
            I: Into<Recipient<M>>,
            F: 'static + Future<Item=I, Error=Error>
    {
        let fut = fut.map(|i| i.into());
        let fut: Box<Future<Item=Recipient<M>, Error=Error>> = Box::new(fut);
        let shared = fut.shared();
        let shared2 = shared.clone();
        Arbiter::spawn(
            shared2
                .map_err(|_| ())
                .and_then(|recip| {
                    send(AddRoute(recip.deref().clone()))
                        .map(|_| ())
                        .map_err(|_| ())
                })
        );
        Self {
            fut: shared,
        }.start()
    }
}

impl<M> Actor for PendingRoute<M>
    where M: SoarMessage
{
    type Context = actix::Context<Self>;
}

impl<M> Handler<M> for PendingRoute<M>
    where M: SoarMessage,
{
    type Result = SoarResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        let fut = self.fut.clone()
                          .map_err(|_| Error::from(RouterError::default()))
                          .and_then(move |recip| 
                            recip
                                .deref().clone()
                                .send(msg)
                                .map_err(Error::from));
        SoarResponse::from(fut)
    }
}


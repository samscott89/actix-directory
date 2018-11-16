use ::actix::dev::*;
use failure::Error;
use futures::{Future};
use serde::{de::DeserializeOwned, Serialize};

use std::any::Any;

use super::*;

pub(crate) struct AddIntoHandler<M, H>
    where M: SoarMessage,
          H: IntoHandler<M>
{
    pub handler: H,
    pub _type: ::std::marker::PhantomData<M>,
}

impl<M, H> Message for AddIntoHandler<M, H>
    where M: SoarMessage,
          H: IntoHandler<M>
{
    type Result = ();
}

pub(crate) struct AddHandler<M>
    where M: SoarMessage,
{
    pub handler: Recipient<M>,
}

impl<M> Message for AddHandler<M>
    where M: SoarMessage,
{
    type Result = ();
}

pub trait IntoHandler<M>: 'static + Send + Sized
    where M: SoarMessage,
{
    type Handler: RequestHandler<M>;

    fn init(self, service: Addr<Service>) -> Box<Future<Item=Self::Handler, Error=()>>;

    #[doc(hidden)]
    fn spawn_init(self, service: Addr<Service>) -> Box<Future<Item=Box<Any>, Error=()>> {
        let service2 = service.clone();
        Box::new(self.init(service.clone()).map(move |h| {
            let recip = Arbiter::start(move |_| {
                SoarActor {
                    inner: Box::new(h),
                    service: service.clone(),
                }
            }).recipient::<M>();
            Arbiter::spawn(service2.send(AddHandler { handler: recip.clone() }).map_err(|_| ()));
            Box::new(recip) as Box<Any>
        }))
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

pub struct SoarActor<M: SoarMessage> {
    inner: Box<RequestHandler<M>>,
    service: Addr<Service>,
}

impl<M: SoarMessage> SoarActor<M> {
    pub fn run<H: 'static +  RequestHandler<M>>(handler: H, service: Addr<Service>) -> Recipient<M> {
        Arbiter::start(move |_| {
            Self {
                inner: Box::new(handler),
                service: service.clone(),
            }
        }).recipient()
    }
}

impl<M: SoarMessage> Handler<M> for SoarActor<M> {
    type Result = SoarResponse<M>;
    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        SoarResponse(self.inner.handle_request(msg, self.service.clone()))
    }
}

impl<M> Actor for SoarActor<M>
    where M: SoarMessage
{
    type Context = Context<Self>;
}


/// Wrapper type for a response to a `SoarMessage`. 
/// Since this implements `actix::MessageResponse`, we can use it as the return type from
/// a `Handler` implementation. 
pub struct SoarResponse<M: SoarMessage>(pub Box<Future<Item=M::Response, Error=Error>>);

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

/// Convenience type for return type of `RequestHandler<M>`.
pub type RespFuture<M> = Box<Future<Item=<M as SoarMessage>::Response, Error=Error>>;

/// A `RequestHandler` is effectively a simplified version of the `actix::Handler` trait.
/// It does not need to know about the context, and instead provides a reference to the 
/// `Service` it is running on, to allow arbitrary other queries to be chained.
///
/// An additional error `Error` is added to the return type of the `Message::Response`,
/// since the meta-service-handler `Service` can also fail.
pub trait RequestHandler<M>: Send
    where M: SoarMessage
{
    fn handle_request(&mut self, msg: M, service: Addr<Service>) -> RespFuture<M>;
}

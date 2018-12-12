use ::actix::dev::*;
use failure::Error;
use futures::Future;
use log::*;
use serde::{de::DeserializeOwned, Serialize};

use crate::get_type;

mod router;

use self::router::{Router, AddRoute, AddRouteFuture, RemoveRoute};

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

/// Register this recipient (usually `Addr<Actor>`) as handling a given message type
pub fn add_route<M, R>(handler: R)
    where M: SoarMessage,
          R: Into<Recipient<M>>
{
    trace!("Add handler for {:?}", get_type!(M));
    send_spawn(AddRoute(handler.into()))
}

/// Set the completion of the future to handle messages of type `M`.
/// Any messages for this address in the meantime will be chained
/// onto the future.
pub fn add_route_fut<M, R, F>(fut: F)
    where M: SoarMessage,
          R: Into<Recipient<M>>,
          F: 'static + Future<Item=R, Error=()> + Send,
{
    send_spawn(AddRouteFuture { fut: fut.map(|r| r.into()) });
}

/// Delete the route
pub fn del_route<M>()
    where M: SoarMessage
{
    send_spawn(RemoveRoute(std::marker::PhantomData::<M>));
}

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
//! How to handle routes which are returned by a future.

use ::actix::dev::*;
use failure::Error;
use futures::{future, Future};

use crate::{MessageExt, FutResponse};
use super::RouterError;

/// To add a `Future`, the `PendingRoute` wrapper handles a number of tasks:
/// - Scheduling incoming messages to be handled once the future resolves.
/// - Add the resolved recipient to the routing table
/// This is done through the `Routeable` implementation.
#[derive(Clone)]
pub struct PendingRoute<M>
    where M: MessageExt,
{
    pub(crate) fut: future::Shared<Box<Future<Item=Recipient<M>, Error=Error> + Send>>
}

impl<M> Actor for PendingRoute<M>
    where M: MessageExt,
{
    type Context = actix::Context<Self>;
}

impl<M> PendingRoute<M>
    where M: MessageExt,
{
    pub fn new<F, I>(fut: F) -> Self
        where 
            I: Into<Recipient<M>>,
            F: 'static + Future<Item=I, Error=Error> + Send
    {
        let fut: Box<Future<Item=Recipient<M>, Error=Error> + Send> = Box::new(fut.map(|r| r.into()));
        let shared = fut.shared();

        Self {
            fut: shared,
        }
    }
}

impl<M> Handler<M> for PendingRoute<M>
    where 
        M: MessageExt,
{
    type Result = FutResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        let fut = self.fut.clone()
                          .map_err(|_| Error::from(RouterError::default()))
                          .and_then(move |recip| 
                            recip.clone()
                                .send(msg)
                                .map_err(Error::from));
        FutResponse::from(fut)
    }
}
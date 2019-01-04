//! How to handle routes which are returned by a future.

use ::actix::dev::*;
use failure::Error;
use futures::{future, future::Either, Future};

use std::marker::PhantomData;

use crate::{app, MessageExt, FutResponse, Routeable, RouteType};
use super::RouterError;

/// To add a `Future`, the `PendingRoute` wrapper handles a number of tasks:
/// - Scheduling incoming messages to be handled once the future resolves.
/// - Add the resolved recipient to the routing table
/// This is done through the `Routeable` implementation.
pub struct PendingRoute<R, M>
    where
        R: 'static + Routeable<M>,
        M: MessageExt,
{
    pub(crate) fut: future::Shared<Box<Future<Item=R, Error=Error> + Send>>,
    ty: Option<RouteType>,
    _marker: PhantomData<M>,
}

impl<R, M> Clone for PendingRoute<R, M>
    where
        R: 'static + Routeable<M>,
        M: MessageExt,
{
    fn clone(&self) -> Self {
        PendingRoute {
            fut: self.fut.clone(),
            ty: self.ty,
            _marker: PhantomData,
        }
    }
}

impl<R, M> Actor for PendingRoute<R, M>
    where
        R: 'static + Routeable<M>,
        M: MessageExt,
{
    type Context = actix::Context<Self>;
}

impl<R, M> PendingRoute<R, M>
    where
        R: 'static + Routeable<M>,
        M: MessageExt,
{
    pub fn new<F>(fut: F) -> Self
        where
            F: 'static + Future<Item=R, Error=Error> + Send
    {
        let fut: Box<Future<Item=R, Error=Error> + Send> = Box::new(fut.map(|r| r.into()));
        let shared = fut.shared();

        Self {
            fut: shared,
            ty: None,
            _marker: PhantomData,
        }
    }

    pub fn set_type(mut self, ty: RouteType) -> Self {
        self.ty.replace(ty);
        self
    }
}

impl<R, M> Handler<M> for PendingRoute<R, M>
    where
        R: 'static + Routeable<M>,
        M: MessageExt,
{
    type Result = FutResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        let ty = self.ty;
        let fut = self.fut.clone()
                          .map_err(|_| Error::from(RouterError::default()))
                          .and_then(move |_| {
                            match ty {
                                Some(RouteType::Client)   => Either::A(Either::A(app::send_local(msg))),
                                Some(RouteType::Server)   => Either::A(Either::B(app::send_in(msg))),
                                Some(RouteType::Upstream) => Either::B(Either::A(app::send_out(msg))),
                                None                      => Either::B(Either::B(app::send(msg))),
                            }
                          });
        FutResponse::from(fut)
    }
}
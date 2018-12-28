use ::actix::dev::*;
use failure::Error;
use futures::{Future, IntoFuture};
use serde_derive::{Deserialize, Serialize};
use url::Url;

use std::ops::Deref;
use std::sync::RwLock;

use crate::{SoarMessage, SoarResponse, StringifiedMessage};
use crate::{get_type, router, service};
use crate::router::{Router, Upstream};

thread_local!(
    /// Each thread maintains its own App struct, which is basically
    /// routing information for messages
    pub(crate) static APP: RwLock<App> = RwLock::new(App::default())
);

#[derive(Clone, Default)]
pub struct App {
    client: Router,
    server: Router,
    upstream: Upstream,
}

impl Actor for App {
    type Context = Context<Self>;
}

impl App {
    pub fn new() -> Self {
        let client = Router::new();
        let server = Router::new();
        let upstream = Upstream::new();
        Self {
            client, server, upstream
        }
    }

    /// Add a single route to the application
    pub fn route<M, R>(&mut self, service: R, ty: RouteType) -> &mut Self
        where R: Routeable<M>,
              M: SoarMessage,
    {
        service.route(self, ty);
        self

    }

    /// Add a service to the application, usually encapsulates mutliple routes
    pub fn service<S: service::Service>(&mut self, service: S) -> &mut Self {
        service.add_to(self)
    }

    /// Internal helper method to insert by route type
    pub(crate) fn add_recip<M, R>(&mut self, service: R, ty: RouteType) -> &mut Self
        where M: SoarMessage,
              R: 'static + Into<Recipient<M>>,
    {
        log::trace!("Add route: {:?} -> {:?} on {:?}", get_type!(M), get_type!(R), ty);
        match ty {
            RouteType::Client => {
                self.client.insert(service.into());
            },
            RouteType::Server => {
                self.server.insert(service.into());
            },
            RouteType::Upstream => {
                // unimplemented!()
            },
        };
        self
    }


    /// Internal helper method to insert by route type
    pub(crate) fn add_str<R>(&mut self, id: &str, service: R, ty: RouteType) -> &mut Self
        where R: 'static + Into<Recipient<crate::StringifiedMessage>>,
    {
        log::trace!("Add route: {:?} -> {:?} on {:?}", id, get_type!(R), ty);

        match ty {
            RouteType::Client => {
                self.client.insert_str(id, service.into());
            },
            RouteType::Server => {
                self.server.insert_str(id, service.into());
            },
            RouteType::Upstream => {
                // unimplemented!()
            },
        };
        self
    }

    pub fn make_current(&self) {
        APP.with(|app| *app.write().unwrap() = self.clone());
    }

    fn send_local<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: SoarMessage
    {
        self.client.send(msg)
    }

    fn send_in<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: SoarMessage
    {
        self.server.send(msg)
    }

    fn send_out<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: SoarMessage
    {
        self.upstream.send(msg)
    }
}

pub fn send<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: SoarMessage
{
    send_out(msg)
}

pub fn send_local<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: SoarMessage
{
    APP.with(|app| {
        app.read().unwrap().send_local(msg)
    })
}


pub fn send_in<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: SoarMessage
{
    APP.with(|app| {
        app.read().unwrap().send_in(msg)
    })
}


pub fn send_out<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: SoarMessage
{
    APP.with(|app| {
        app.read().unwrap().send_out(msg)
    })
}



#[derive(Default)]
struct Passthrough;

impl Actor for Passthrough {
    type Context = Context<Self>;
}

impl<M> Handler<M> for Passthrough
    where M: SoarMessage,
{
    type Result = SoarResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        let res = send_out(msg).map_err(Error::from);
        SoarResponse(Box::new(res))
    }
}

pub fn no_client<M: SoarMessage>() -> Recipient<M> {
    Passthrough::start_default().recipient()
}

#[derive(Default)]
struct EmptyServer;

impl Actor for EmptyServer {
    type Context = Context<Self>;
}

impl<M> Handler<M> for EmptyServer
    where M: SoarMessage,
{
    type Result = SoarResponse<M>;

    fn handle(&mut self, _msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        SoarResponse::from(
            Err(
                router::RouterError::default().into()
            ).into_future()
        )
    }
}


pub fn no_server<M: SoarMessage>() -> Recipient<M> {
    EmptyServer::start_default().recipient()
}

/// Internal messages are the default type for 
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Internal<M: SoarMessage>(
    #[serde(bound = "M: SoarMessage")]
    pub M
);

impl<M: SoarMessage> Message for Internal<M> {
    type Result = M::Result;
}

impl<M: SoarMessage> SoarMessage for Internal<M> {
    type Response = <M as Message>::Result;
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Inbound<M: SoarMessage>(
    #[serde(bound = "M: SoarMessage")]
    pub M
);

impl<M: SoarMessage> Message for Inbound<M> {
    type Result = M::Result;
}

impl<M: SoarMessage> SoarMessage for Inbound<M> {
    type Response = <M as Message>::Result;
}


#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Outbound<M: SoarMessage>(
    #[serde(bound = "M: SoarMessage")]
    pub M
);

impl<M: SoarMessage> Message for Outbound<M> {
    type Result = M::Result;
}

impl<M: SoarMessage> SoarMessage for Outbound<M> {
    type Response = <M as Message>::Result;
}


#[derive(Copy, Clone, Debug)]
pub enum RouteType {
    Client,
    Server,
    Upstream,
}

trait Sealed { }

pub trait Routeable<M> // + Sealed
    where M: SoarMessage
{
    fn route(self, app: &mut App, ty: RouteType) -> &mut App;
}

impl<M> Routeable<M> for Recipient<M>
    where M: SoarMessage,
{
    fn route(self, app: &mut App, ty: RouteType) -> &mut App {
        app.add_recip(self, ty)
    }
}

impl<A, M> Routeable<M> for Addr<A>
    where M: SoarMessage,
          A: Actor<Context=Context<A>> + Handler<M>,
{
    fn route(self, app: &mut App, ty: RouteType) -> &mut App {
        app.add_recip(self, ty)
    }
}

impl<M> Routeable<M> for router::PendingRoute<M>
    where
        M: SoarMessage,
{
    fn route(self, app: &mut App, ty: RouteType) -> &mut App {
        Arbiter::spawn(self.fut.clone().map(move |recip| {
            let recip = recip.deref().clone();
            crate::app::APP.with(move |app| {
                app.write().unwrap().add_recip(recip, ty);
            });
        }).map_err(|_| ()));
        app.add_recip(self.start().recipient(), ty)
    }
}

impl<R, M> Routeable<M> for R
    where M: SoarMessage,
          R: Into<router::Remote>
{
    fn route(self, app: &mut App, ty: RouteType) -> &mut App {
        if let RouteType::Upstream = ty {
            app.upstream.insert::<M>(self.into());
        }
        app
    }
}

impl Routeable<StringifiedMessage> for (&str, Recipient<StringifiedMessage>)
{
    fn route(self, app: &mut App, ty: RouteType) -> &mut App {
        app.add_str(self.0, self.1, ty)
    }
}

impl<A> Routeable<StringifiedMessage> for (&str, Addr<A>)
    where A: Actor<Context=Context<A>> + Handler<StringifiedMessage>,
{
    fn route(self, app: &mut App, ty: RouteType) -> &mut App {
        app.add_str(self.0, self.1.recipient(), ty)
    }
}

impl Routeable<StringifiedMessage> for (&str, router::PendingRoute<StringifiedMessage>)
{
    fn route(self, app: &mut App, ty: RouteType) -> &mut App {
        let id = self.0.to_string();
        Arbiter::spawn(self.1.fut.clone().map(move |recip| {
            let recip = recip.deref().clone();
            crate::app::APP.with(move |app| {
                app.write().unwrap().add_str(&id, recip, ty);
            });
        }).map_err(|_| ()));
        app.add_str(self.0, self.1.start().recipient(), ty)
    }
}

impl<R> Routeable<StringifiedMessage> for (&str, R)
    where R: Into<router::Remote>
{
    fn route(self, app: &mut App, ty: RouteType) -> &mut App {
        if let RouteType::Upstream = ty {
            app.upstream.insert_str(self.0, self.1.into());
        }
        app
    }
}


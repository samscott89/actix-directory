use ::actix::dev::*;
use actix::msgs::Execute;
use failure::{Error, Fail};
use futures::{future, future::Either, Future};
use log::*;
use serde_derive::{Deserialize, Serialize};
use url::Url;

use crate::router;
use crate::router::*;

pub struct Service {
    client: Addr<Arbiter>,
    server: Addr<Arbiter>,
}

impl Actor for Service {
    type Context = Context<Self>;
}

impl std::default::Default for Service {
    fn default() -> Self {
        Self::new()
    }
}

impl actix::Supervised for Service { }
impl SystemService for Service { }

pub fn no_client<M: SoarMessage>() -> Recipient<M> {
    Passthrough::start_default().recipient()
}

impl Service {
    pub fn new() -> Self {
        let client = Arbiter::new("soar_service_client");
        let server = Arbiter::new("soar_service_server");
        Self {
            client, server,
        }
    }

    pub fn with_client<F>(&self, f: F)
        where F: 'static + Send + FnOnce()
    {
        self.client.do_send(Execute::new(move || -> Result<(), ()> {
            f();
            Ok(())
        }));
    }

    pub fn with_server<F>(&self, f: F)
        where F: 'static + Send + FnOnce()
    {
        self.server.do_send(Execute::new(move || -> Result<(), ()> {
            f();
            Ok(())
        }));
    }

    pub fn add_service<M, RC, RS>(self, client: RC, service: RS) -> Self
        where M: SoarMessage,
              RC: Into<Recipient<M>> + Send + 'static,
              RS: Into<Recipient<M>> + Send + 'static,
    {
        self.with_client(|| {
            add_route(client);
        });
        self.with_server(|| {
            add_route(service);
        });
        self
    }

    pub fn add_http_service<M, RC>(self, client: RC, url: Url) -> Self
        where M: SoarMessage,
              RC: Into<Recipient<M>> + Send + 'static,
    {
        self.with_client(||  {
            add_route(client);
        });
        self.with_server(|| {
            add_route::<M, _>(crate::http::HttpHandler::from(url).start());
        });
        self
    }

    pub fn run(self) -> Addr<Self> {
        let addr = self.start();
        System::current().registry().set(addr.clone());
        addr
    }
}

pub fn send<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: SoarMessage
{
    send_local(msg)
}


pub fn send_local<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: SoarMessage
{
    System::current().registry().get::<Service>()
        .send(Local(msg))
        .map_err(Error::from)
}


pub fn send_remote<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: SoarMessage
{
    System::current().registry().get::<Service>()
        .send(Remote(msg))
        .map_err(Error::from)
}

impl<M> Handler<Local<M>> for Arbiter
    where M: SoarMessage
{
    type Result = SoarResponse<Local<M>>;

    fn handle(&mut self, msg: Local<M>, _ctxt: &mut Context<Self>) -> Self::Result {
        SoarResponse::from(router::send(msg.0))
    }
}

impl<M> Handler<Remote<M>> for Arbiter
    where M: SoarMessage
{
    type Result = SoarResponse<Remote<M>>;

    fn handle(&mut self, msg: Remote<M>, _ctxt: &mut Context<Self>) -> Self::Result {
        SoarResponse::from(router::send(msg.0))
    }
}

impl<M> Handler<Local<M>> for Service
    where M: SoarMessage, 
{
    type Result = SoarResponse<Local<M>>;

    fn handle(&mut self, msg: Local<M>, _ctxt: &mut Context<Self>) -> Self::Result {
        let res = self.client.send(msg).map_err(Error::from);
        SoarResponse::from(res)
    }
}



impl<M> Handler<Remote<M>> for Service
    where M: SoarMessage
{
    type Result = SoarResponse<Remote<M>>;

    fn handle(&mut self, msg: Remote<M>, _ctxt: &mut Context<Self>) -> Self::Result {
        let res = self.server.send(msg).map_err(Error::from);
        SoarResponse::from(res)
    }
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
        let res = send_remote(msg).map_err(Error::from);
        SoarResponse(Box::new(res))
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct Local<M: SoarMessage>(
    #[serde(bound = "M: SoarMessage")]
    M
);

impl<M: SoarMessage> Message for Local<M> {
    type Result = M::Result;
}

impl<M: SoarMessage> SoarMessage for Local<M> {
    type Response = <M as Message>::Result;
}

#[derive(Debug, Deserialize, Serialize)]
struct Remote<M: SoarMessage>(
    #[serde(bound = "M: SoarMessage")]
    M
);

impl<M: SoarMessage> Message for Remote<M> {
    type Result = M::Result;
}

impl<M: SoarMessage> SoarMessage for Remote<M> {
    type Response = <M as Message>::Result;
}


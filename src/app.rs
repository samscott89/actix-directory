use ::actix::dev::*;
use failure::Error;
use futures::{Future, IntoFuture};
use serde_derive::{Deserialize, Serialize};
use url::Url;


use crate::{SoarMessage, SoarResponse};
use crate::router;
use crate::router::{Router, Upstream};

#[derive(Default)]
pub struct App {
    client: Router,
    server: Router,
    upstream: Upstream,
}

impl Actor for App {
    type Context = Context<Self>;
}


impl actix::Supervised for App { }
impl SystemService for App { }

impl App {
    pub fn new() -> Self {
        let client = Router::new();
        let server = Router::new();
        let upstream = Upstream::new();
        Self {
            client, server, upstream
        }
    }

    pub fn add_client<M, R>(mut self, client: R) -> Self
        where M: SoarMessage,
              R: Into<Recipient<M>> + 'static,
    {
        self.client.insert(client.into());
        self
    }

    pub fn add_server<M, R>(mut self, server: R) -> Self
        where M: SoarMessage,
              R: Into<Recipient<M>> +  'static,
    {
        self.server.insert(server.into());
        self
    }

    pub fn add_http_remote<M>(mut self, url: Url) -> Self
        where M: SoarMessage,
    {
        self.upstream.insert_http::<M>(url);
        self
    }

    pub fn run(self) -> Addr<Self> {
        let addr = self.start();
        System::current().registry().set(addr.clone());
        addr
    }
}


impl<M> Handler<Internal<M>> for App
    where M: SoarMessage
{
    type Result = SoarResponse<Internal<M>>;

    fn handle(&mut self, msg: Internal<M>, _ctxt: &mut Context<Self>) -> Self::Result {
        SoarResponse::from(self.client.send(msg.0))
    }
}

impl<M> Handler<Inbound<M>> for App
    where M: SoarMessage
{
    type Result = SoarResponse<Inbound<M>>;

    fn handle(&mut self, msg: Inbound<M>, _ctxt: &mut Context<Self>) -> Self::Result {
        SoarResponse::from(self.server.send(msg.0))
    }
}

impl<M> Handler<Outbound<M>> for App
    where M: SoarMessage
{
    type Result = SoarResponse<Outbound<M>>;

    fn handle(&mut self, msg: Outbound<M>, _ctxt: &mut Context<Self>) -> Self::Result {
        SoarResponse::from(self.upstream.send(msg.0))
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
    System::current().registry().get::<App>()
        .send(Internal(msg))
        .map_err(Error::from)
}


pub fn send_in<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: SoarMessage
{
    System::current().registry().get::<App>()
        .send(Inbound(msg))
        .map_err(Error::from)
}


pub fn send_out<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: SoarMessage
{
    System::current().registry().get::<App>()
        .send(Outbound(msg))
        .map_err(Error::from)
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


use ::actix::dev::*;
use failure::Error;
use futures::{Future, IntoFuture};
use serde_derive::{Deserialize, Serialize};
use url::Url;

use std::sync::RwLock;

use crate::{SoarMessage, SoarResponse};
use crate::router;
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

    pub fn add_client<M, F, R>(&mut self, client: F) -> &mut Self
        where M: SoarMessage,
              F: IntoFuture<Item=R, Error=Error>,
              F::Future: 'static + Send,
              R: Into<Recipient<M>> + 'static,
    {
        let fut = client.into_future().map(|client| {
            let client: Recipient<M> = client.into();
            let client2 = client.clone();
            APP.with(move |app| {
                app.write().unwrap().add_client::<M, _, _>(Ok(client2));
            });
            client
        });
        let client = router::PendingRoute::new(fut);
        self.client.insert(client.start().recipient());
        self
    }

    pub fn add_client_str<R>(&mut self, id: &str, client: R) -> &mut Self
        where R: Into<Recipient<crate::StringifiedMessage>>
    {
        self.client.insert_str(id, client.into());
        self
    }

    pub fn add_server<M, F, R>(&mut self, server: F) -> &mut Self
        where M: SoarMessage,
              F: IntoFuture<Item=R, Error=Error>,
              F::Future: 'static + Send,
              R: Into<Recipient<M>> + 'static,

    {
        let fut = server.into_future().map(|server| {
            let server: Recipient<M> = server.into();
            let server2 = server.clone();
            APP.with(move |app| {
                app.write().unwrap().add_server::<M, _, _>(Ok(server2));
            });
            server
        });
        let server = router::PendingRoute::new(fut);
        self.server.insert(server.start().recipient());
        self
    }

    pub fn add_server_str<R>(&mut self, id: &str, client: R) -> &mut Self
        where R: Into<Recipient<crate::StringifiedMessage>>
    {
        self.server.insert_str(id, client.into());
        self
    }

    pub fn add_http_remote<M>(&mut self, url: Url) -> &mut Self
        where M: SoarMessage,
    {
        self.upstream.insert_http::<M>(url);
        self
    }

    pub fn add_http_remote_str<R>(&mut self, id: &str, url: Url) -> &mut Self
    {
        self.upstream.insert_http_str(id, url);
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

pub fn no_client<M: SoarMessage>() -> Result<Recipient<M>, Error> {
    Ok(Passthrough::start_default().recipient())
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


pub fn no_server<M: SoarMessage>() -> Result<Recipient<M>, Error> {
    Ok(EmptyServer::start_default().recipient())
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


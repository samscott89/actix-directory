//! The main `actix_directory` functionality.

use ::actix::dev::*;
use failure::Error;
use futures::{Future, IntoFuture};

use std::cell::RefCell;
use std::ops::Deref;
// use std::sync::RwLock;

use crate::{MessageExt, FutResponse, StringifiedMessage};
use crate::{get_type, router, service};
use crate::router::{Router, Upstream};

thread_local!(
    /// Each thread maintains its own `App` struct, which is basically
    /// routing information for messages
    pub(crate) static APP: RefCell<App> = RefCell::new(App::default())
);

/// An application can be seen as a set of independent services, connecting
/// together through the external routes.
///
/// # Usage
///
/// First construct an application by creating a few routes/services
///
/// ```ignore
/// let app = App::new().add_route::<TestMessage, _>(addr.clone(), RouteType::Server);
/// ```
///
/// In order for most applications to work correcty, you should set this to be the current app,
/// `app.make_current()`, and then can send messages via `actix_directory::app::send(...)`.
#[derive(Default)]
pub struct App {
    client: Router,
    server: Router,
    upstream: Upstream,
}

impl Actor for App {
    type Context = Context<Self>;
}

impl App {
    /// Create a new application with empty routing tables
    pub fn new() -> Self {
        let client = Router::with_name("client");
        let server = Router::with_name("server");
        let upstream = Upstream::new();
        Self {
            client, server, upstream
        }
    }

    /// Add a single route to the application
    ///
    /// The `service` should be anythign which implements `Routable`.
    /// This can either be an `Addr<A>` for some actor `A: Actor + Handler<M>`, 
    /// or a `Url` in the case of an upstream route, for example.
    ///
    /// For non-typed routes, you can provide the string identifier
    /// as `(service, "type_name").
    ///
    /// Note that generally you will need to specify the message type
    /// i.e `app.route::<SomeMessage, _>(addr, RouteType::Client)`.
    /// Except for when specifying stringly typed message extensions,
    /// in which case the type can be inferred to be `StringifiedMessage`.
    pub fn route<M, R>(mut self, service: R, ty: RouteType) -> Self
        where R: Routeable<M>,
              M: MessageExt,
    {
        service.route(&mut self, ty);
        self
    }

    /// Add a service to the application, usually encapsulates mutliple routes
    pub fn service<S: service::Service>(self, service: S) -> Self {
        service.add_to(self)
    }

    /// Internal helper method to insert by route type
    pub(crate) fn add_recip<M, R>(&mut self, service: R, ty: RouteType) -> &mut Self
        where M: MessageExt,
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
                log::error!("Attempted to add a local service as an upstream route. This isn't currently supported");
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
                log::error!("Attempted to add a local service as an upstream route. This isn't currently supported");
            },
        };
        self
    }

    /// Set this application to be the current application default.
    pub fn make_current(self) {
        APP.with(|app| app.replace(self));
    }

    /// Send a message on the default channel (a local message via the client)
    pub fn send<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: MessageExt
    {
        self.send_local(msg)
    }

    /// Send a message to the local handler
    pub fn send_local<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: MessageExt
    {
        self.client.send(msg)
    }

    /// Send a message to the handler for incoming messages
    pub fn send_in<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: MessageExt
    {
        self.server.send(msg)
    }


    /// Send a message to the handler for outgoing messages
    pub fn send_out<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: MessageExt
    {
        self.upstream.send(msg)
    }
}

/// Send a message on the default channel (a local message via the client)
pub fn send<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: MessageExt
{
    APP.with(|app| {
        app.borrow().send(msg)
    })}

/// Send a message to the local handler
pub fn send_local<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: MessageExt
{
    APP.with(|app| {
        app.borrow().send_local(msg)
    })
}

/// Send a message to the handler for incoming messages
pub fn send_in<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: MessageExt
{
    APP.with(|app| {
        app.borrow().send_in(msg)
    })
}

/// Send a message to the handler for outgoing messages
pub fn send_out<M>(msg: M) -> impl Future<Item=M::Response, Error=Error>
    where M: MessageExt
{
    APP.with(|app| {
        app.borrow().send_out(msg)
    })
}



#[derive(Default)]
struct Passthrough;

impl Actor for Passthrough {
    type Context = Context<Self>;
}

impl<M> Handler<M> for Passthrough
    where M: MessageExt,
{
    type Result = FutResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        let res = send_out(msg).map_err(Error::from);
        FutResponse(Box::new(res))
    }
}

/// This is a simple client implementation which will just forward all
/// messages to the upstream handler.
pub fn no_client<M: MessageExt>() -> Recipient<M> {
    Passthrough::start_default().recipient()
}

#[derive(Default)]
struct EmptyServer;

impl Actor for EmptyServer {
    type Context = Context<Self>;
}

impl<M> Handler<M> for EmptyServer
    where M: MessageExt,
{
    type Result = FutResponse<M>;

    fn handle(&mut self, _msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        FutResponse::from(
            Err(
                router::RouterError::default().into()
            ).into_future()
        )
    }
}

/// Create a simple non-existent server implementation which rejects all messages.
/// While this is not strictly necessary, since a missing route has a similar effect,
/// it is easier for debugging purposes to explicitly have a request rejector.
pub fn no_server<M: MessageExt>() -> Recipient<M> {
    EmptyServer::start_default().recipient()
}

/// When creating a new route, the type is used to specify what kind of route it is.
///
/// A `Client` is a local handler to complete a request. This may include making various upstream
/// requests and composing the outputs in some way. For example, a `UserInfo` request might
/// need to correlate data from a database, as well as some other APIs. On the other hand, `UserInfo` might
/// only need to talk to an upstream server which returns the desired object, but the client might
/// still want to do some caching or connection pooling.
///
/// A `Server` is a local handler which responds to incoming messages. For example, the aforementioned
/// server handling `UserInfo` requests. Although the server may also need to talk to additional resources,
/// the implication is that the sever is the endpoint.
///
/// An `Upstream` is remote address for a `Server`. This can be either an HTTP/REST endpoint, or (coming soon)
/// an RPC endpoint.
///
#[derive(Copy, Clone, Debug)]
pub enum RouteType {
    Client,
    Server,
    Upstream,
}

/// Trait to encapsulate anything which can be used as a actix_directory service to
/// respond handle messages.
///
/// The `route` method is used for the `Routeable` to add itself to the application,
/// in the appropriate way. For example, `Recipient<M>` is the core type for the routing table,
/// but `Routeable` can be `Url`s, `Future`s, or even `(&str, Recipient)`.
pub trait Routeable<M>
    where M: MessageExt
{
    fn route(self, app: &mut App, ty: RouteType);
}

impl<M> Routeable<M> for Recipient<M>
    where M: MessageExt,
{
    fn route(self, app: &mut App, ty: RouteType) {
        app.add_recip(self, ty);
    }
}

impl<A, M> Routeable<M> for Addr<A>
    where M: MessageExt,
          A: Actor<Context=Context<A>> + Handler<M>,
{
    fn route(self, app: &mut App, ty: RouteType)  {
        app.add_recip(self, ty);
    }
}

impl<M> Routeable<M> for router::PendingRoute<M>
    where
        M: MessageExt,
{
    fn route(self, app: &mut App, ty: RouteType)  {
        Arbiter::spawn(self.fut.clone().map(move |recip| {
            let recip = recip.deref().clone();
            crate::app::APP.with(move |app| {
                app.borrow_mut().add_recip(recip, ty);
            });
        }).map_err(|_| ()));
        app.add_recip(self.start().recipient(), ty);
    }
}

impl<R, M> Routeable<M> for R
    where M: MessageExt,
          R: Into<router::Remote>
{
    fn route(self, app: &mut App, ty: RouteType)  {
        if let RouteType::Upstream = ty {
            app.upstream.insert::<M>(self.into());
        }
    }
}

impl Routeable<StringifiedMessage> for (&str, Recipient<StringifiedMessage>)
{
    fn route(self, app: &mut App, ty: RouteType)  {
        app.add_str(self.0, self.1, ty);
    }
}

impl<A> Routeable<StringifiedMessage> for (&str, Addr<A>)
    where A: Actor<Context=Context<A>> + Handler<StringifiedMessage>,
{
    fn route(self, app: &mut App, ty: RouteType)  {
        app.add_str(self.0, self.1.recipient(), ty);
    }
}

impl Routeable<StringifiedMessage> for (&str, router::PendingRoute<StringifiedMessage>)
{
    fn route(self, app: &mut App, ty: RouteType)  {
        let id = self.0.to_string();
        Arbiter::spawn(self.1.fut.clone().map(move |recip| {
            let recip = recip.deref().clone();
            crate::app::APP.with(move |app| {
                app.borrow_mut().add_str(&id, recip, ty);
            });
        }).map_err(|_| ()));
        app.add_str(self.0, self.1.start().recipient(), ty);
    }
}

impl<R> Routeable<StringifiedMessage> for (&str, R)
    where R: Into<router::Remote>
{
    fn route(self, app: &mut App, ty: RouteType)  {
        if let RouteType::Upstream = ty {
            app.upstream.insert_str(self.0, self.1.into());
        }
    }
}


//! The main `actix_directory` functionality.

use ::actix::dev::*;
use failure::Error;
use futures::{future, Future, IntoFuture};
use log::*;
use serde::{Deserialize, Serialize};

use std::cell::RefCell;
use std::ops::Deref;
#[cfg(unix)]
use std::os::unix::net::{UnixStream, UnixListener};
#[cfg(unix)]
use std::path::PathBuf;
use std::sync::Arc;

use crate::prelude::*;
use crate::{get_type, router, service};
use crate::http::HttpFactory;
use crate::router::Router;

thread_local!(
    /// Each thread maintains its own `App` struct, which is basically
    /// routing information for messages
    pub(crate) static APP: Arc<RefCell<App>> = Arc::new(RefCell::new(App::default()))
);

thread_local!(
    /// Each thread maintains its own `App` struct, which is basically
    /// routing information for messages
    pub(crate) static SOCKET_DIR: tempfile::TempDir = tempfile::tempdir().unwrap();
);

#[cfg(unix)]
pub(crate) fn sock_path(name: &str) -> PathBuf {
    // let tmp_dir = tempdir::TempDir::new("actix-dir").unwrap();
    SOCKET_DIR.with(|dir| dir.path().join(format!("{}.sock", name)))
}

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
pub struct App {
    client: Router,
    server: Router,
    upstream: Router,
    http: HttpFactory<ServerIn>,
    http_internal: HttpFactory<ClientIn>,
    // rpc: crate::rpc::RpcHandler,
}

impl Actor for App {
    type Context = Context<Self>;
}

impl std::default::Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Create a new application with empty routing tables
    pub fn new() -> Self {
        let http = HttpFactory::new();
        let mut http_internal = HttpFactory::new();
        http_internal.route::<OpaqueMessage>(Some(RouteType::Client));
        let client = Router::with_name("client");
        let server = Router::with_name("server");
        let upstream = Router::with_name("upstream");
        // let addr = ClientIn::start_default();
        // let rpc = crate::rpc::RpcHandler::new(addr);
        Self {
            client, server, upstream, http, http_internal,
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
    /// in which case the type can be inferred to be `OpaqueMessage`.
    pub fn route<M, R>(mut self, service: R, ty: RouteType) -> Self
        where R: Routeable<M>,
              M: MessageExt,
    {
        service.route(&mut self, ty);
        self
    }

    /// Set a default fallback route as an upstream route
    pub fn default_route<R: Into<Remote>>(mut self, remote: R) -> Self {
        self.upstream.default = Some(remote.into().start());
        self
    }

    /// Add a service to the application, usually encapsulates mutliple routes
    pub fn service<S: service::Service>(self, service: S) -> Self {
        service.add_to(self)
    }

    #[cfg(unix)]
    pub fn plugin(self, plugin: crate::Plugin) -> Self {
        plugin.add_to(self)
    }

    /// Expose the message `M` on HTTP endpoint `path`.
    pub fn expose<M>(mut self) -> Self
        where M: MessageExt
    {
        self.http.route::<M>(None);
        self
    }

    // pub fn configure(&self, app: actix_web::App<Addr<app::ServerIn>>) -> actix_web::App<Addr<app::ServerIn>> {
    //     self.http.configure(app)
    // }

    pub fn get_factory(&self) -> HttpFactory<ServerIn> {
        self.http.clone()
    }

    // #[cfg(test)]
    // pub fn get_configure_test<'a>(&self) -> Box<FnOnce(&mut actix_web::test::TestApp<Addr<ServerIn>>) -> &mut actix_web::test::TestApp<Addr<ServerIn>> + Send> {
    //     self.http.factory_test
    // }

    /// Internal helper method to insert by route type
    pub(crate) fn add_recip<M, R>(&mut self, service: R, ty: RouteType) -> &mut Self
        where M: MessageExt,
              R: 'static + Into<Recipient<M>>,
    {
        log::trace!("Add route: {:?} -> {:?} on {:?}", get_type!(M), get_type!(R), ty);
        match ty {
            RouteType::Client => {
                // if let Some(path)= M::PATH {
                //     self.rpc.route::<M>(path);
                // }
                self.http_internal.route::<M>(Some(RouteType::Client));
                self.client.insert(service.into());
            },
            RouteType::Server => {
                self.http_internal.route::<M>(Some(RouteType::Server));
                self.server.insert(service.into());
            },
            RouteType::Upstream => {
                self.upstream.insert(service.into());
            },
        };
        self
    }

    /// Internal helper method to insert by route type
    pub(crate) fn add_str<R>(&mut self, id: &str, service: R, ty: RouteType) -> &mut Self
        where R: 'static + Into<Recipient<crate::OpaqueMessage>>,
    {
        log::trace!("Add route: {:?} -> {:?} on {:?}", id, get_type!(R), ty);

        match ty {
            RouteType::Client => {
                // self.http_internal.str_route(id);
                self.client.insert_str(id, service.into());
            },
            RouteType::Server => {
                self.server.insert_str(id, service.into());
            },
            RouteType::Upstream => {
                self.upstream.insert_str(id, service.into());
            },
        };
        self
    }

    /// Set this application to be the current application default.
    pub fn make_current(self) {
        log::trace!("Setting APP on thread: : {:?}", std::thread::current().id());
        // self.serve_local_http();
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
        if let Some(r) = self.client.recipient_for(&msg) {
            trace!("Found recipient on client");
            future::Either::A(r.send(msg).map_err(Error::from))
        } else {
            trace!("No client, forwarding to server");
            future::Either::B(self.send_in(msg))
        }
    }

    /// Send a message to the handler for incoming messages
    pub fn send_in<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: MessageExt
    {
        if let Some(r) = self.server.recipient_for(&msg) {
            trace!("Found recipient on server");
            future::Either::A(r.send(msg).map_err(Error::from))
        } else {
            trace!("No server, forwarding to upstream");
            future::Either::B(self.send_out(msg))
        }
    }


    /// Send a message to the handler for outgoing messages
    pub fn send_out<M>(&self, msg: M) -> impl Future<Item=M::Response, Error=Error>
        where M: MessageExt
    {
        self.upstream.send(msg)
    }

    /// Helper function to create a new actix_web application with the routes preconfigured
    pub fn http_server(&self) -> impl Fn() -> actix_web::App<Addr<ServerIn>> + Clone {
        let addr = ServerIn::start_default();
        let factory = self.http.clone();
        move || {
            let app = actix_web::App::with_state(addr.clone());
            factory.clone().configure(app)
                .middleware(actix_web::middleware::Logger::default())

        }
    }

    #[cfg(unix)]
    pub fn serve_local_http(&self, path: Option<std::path::PathBuf>) -> std::path::PathBuf {
        let addr = ClientIn::start_default();
        let factory = self.http_internal.clone();
        let path = path.unwrap_or_else(|| sock_path("main"));
        let listener = tokio_uds::UnixListener::bind(&path).unwrap();
        // let fd = listener.into_raw_fd();
        // let tcp_listener = TcpListener::from(fd);
        actix_web::server::new(move || {
            let app = actix_web::App::with_state(addr.clone());
            factory.clone().configure(app)
                .middleware(actix_web::middleware::Logger::default())
        })
        .workers(2)
        // .bind(&format!("0.0.0.0:{}", port))
        .start_incoming(listener.incoming(), false);
        path
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

#[derive(Clone, Debug, Default)]
pub struct ServerIn;

impl Actor for ServerIn {
    type Context = Context<Self>;
}

impl<M> Handler<M> for ServerIn
    where M: MessageExt,
{
    type Result = FutResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        log::trace!("Handling request for server in on thread: {:?}", std::thread::current().id());
        let res = send_in(msg).map_err(Error::from);
        FutResponse(Box::new(res))
    }
}

#[derive(Clone, Debug, Default)]
pub struct ClientIn;

impl Actor for ClientIn {
    type Context = Context<Self>;
}

impl<M> Handler<M> for ClientIn
    where M: MessageExt,
{
    type Result = FutResponse<M>;

    fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        log::trace!("Handling request for Clientin on thread: {:?}", std::thread::current().id());
        let res = send_local(msg).map_err(Error::from);
        FutResponse(Box::new(res))
    }
}

#[derive(Default)]
struct RejectAll;

impl Actor for RejectAll {
    type Context = Context<Self>;
}

impl<M> Handler<M> for RejectAll
    where M: MessageExt,
{
    type Result = FutResponse<M>;

    fn handle(&mut self, _msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
        trace!("Rejecting message from propogating through");
        FutResponse::from(
            Err(
                router::RouterError::default().into()
            ).into_future()
        )
    }
}


/// Create a simple non-existent client implementation which rejects all messages.
/// This is to explicitly avoid forwarded messages along on a missing route.
pub fn no_client<M: MessageExt>() -> Recipient<M> {
    trace!("Create new rejecting client");
    RejectAll::start_default().recipient()
}


/// Create a simple non-existent server implementation which rejects all messages.
/// This is to explicitly avoid forwarded messages along on a missing route.
pub fn no_server<M: MessageExt>() -> Recipient<M> {
    RejectAll::start_default().recipient()
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
#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
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
pub trait Routeable<M>: Clone
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

impl<R, M> Routeable<M> for router::PendingRoute<R>
    where
        R: 'static + Routeable<M>,
        M: MessageExt,
{
    fn route(self, app: &mut App, ty: RouteType)  {
        Arbiter::spawn(self.fut.clone().map(move |route| {
            let route = route.deref().clone();
            crate::app::APP.with(move |app| {
                route.route(&mut app.borrow_mut(), ty);
            });
        }).map_err(|_| ()));
        app.add_recip(self.set_type(ty).start().recipient(), ty);
    }
}

impl<R, M> Routeable<M> for R
    where M: MessageExt,
          R: Into<router::Remote> + Clone
{
    fn route(self, app: &mut App, ty: RouteType)  {
        Routeable::<M>::route(self.into().start(), app, ty)
    }
}

impl Routeable<OpaqueMessage> for (&str, Recipient<OpaqueMessage>)
{
    fn route(self, app: &mut App, ty: RouteType)  {
        app.add_str(self.0, self.1, ty);
    }
}

impl<A> Routeable<OpaqueMessage> for (&str, Addr<A>)
    where A: Actor<Context=Context<A>> + Handler<OpaqueMessage>,
{
    fn route(self, app: &mut App, ty: RouteType)  {
        app.add_str(self.0, self.1.recipient(), ty);
    }
}

impl<R> Routeable<OpaqueMessage> for (&str, R)
    where R: Into<router::Remote> + Clone
{
    fn route(self, app: &mut App, ty: RouteType)  {
        (self.0, self.1.into().start()).route(app, ty)
    }
}

impl<R> Routeable<OpaqueMessage> for (&str, router::PendingRoute<R>)
    where
        for<'a> (&'a str, R): Routeable<OpaqueMessage>,
        R: 'static + Routeable<OpaqueMessage> + Clone
{
    fn route(self, app: &mut App, ty: RouteType)  {
        let id = self.0.to_string();
        Arbiter::spawn(self.1.fut.clone().map(move |route| {
            let route = route.deref().clone();
            crate::app::APP.with(move |app| {
                (id.as_str(), route).route(&mut app.borrow_mut(), ty);
            });
        }).map_err(|_| ()));
        app.add_str(self.0, self.1.set_type(ty).start().recipient(), ty);
    }
}



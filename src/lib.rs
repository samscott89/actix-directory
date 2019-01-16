#![cfg_attr(feature="print_types", feature(core_intrinsics))]


//! actix-directory is designed to enable building distributed applications
//! using the Actix framework.
//!
//! The idea is to abstract services into a client/server model, and then provide simple
//! message handlers for these messages. Such that requests can be made over HTTP/REST or RPC
//! connections.
//!
//! The core of this is a routing table which either maps types to endpoints, or strings to endpoints.
//! The former is for apps with centrally defined messages (i.e. all defined in a single library),
//! whereas the latter is useful for extending functionality.
//!
//! Essentially, actix-directory simplifies sending messages. If an app wants some service to handle
//! a `MessageRequest`, it simply uses `app::send(MessageReq { ... })`, which looks up the correct client
//! implementation (running locally), and makes the request to the remote server.
//!
//! Future ideas include adding service discovery/proxies as a supported endpoint.

pub mod app;
#[cfg(unix)]
pub mod plugin;
pub mod http;
mod router;
// pub mod rpc;
pub mod service;

pub use self::app::{App, Routeable, RouteType};
#[cfg(unix)]
pub use self::plugin::Plugin;
pub use self::router::PendingRoute;

#[cfg(test)]
pub mod test_helpers;

use ::actix::dev::*;
use failure::Error;
use futures::Future;
use serde::{Deserialize, de::DeserializeOwned, Serialize};

pub mod prelude {
	pub use crate::{app, http::HttpApp, router::Remote, service::Service, App, FutResponse, MessageExt, Routeable, RouteType, PendingRoute, OpaqueMessage,};
	#[cfg(unix)]
	pub use crate::Plugin;
}

/// Helper type for messages (`actix::Message`) which can be processed by `soar`.
/// Requires the inputs/outputs to be de/serializable so that they can be sent over
/// a network.
pub trait MessageExt:
    Message<Result=<Self as MessageExt>::Response>
    + 'static + Send + DeserializeOwned + Serialize
    + std::fmt::Debug
{
	const PATH: &'static str;

    type Response: 'static + Send + DeserializeOwned + Serialize;
}

/// Wrapper type for a response to a `MessageExt`. 
/// Since this implements `actix::MessageResponse`, we can use it as the return type from
/// a `Handler` implementation.
///
/// Note, this is often two levels of errors. The outer error encapsulates the directory
/// errors - missing routes, failed connections etc., and the inner errors are from
/// of `M::Response`,  corresponding to the app-level error.
pub struct FutResponse<M: MessageExt>(pub Box<Future<Item=M::Response, Error=Error>>);

impl<F, M> From<F> for FutResponse<M>
    where
        M: MessageExt, 
        F: 'static + Future<Item=M::Response, Error=Error>,
{
    fn from(other: F) -> Self {
        FutResponse(Box::new(other))
    }
}

impl<A, M> MessageResponse<A, M> for FutResponse<M>
    where 
        A: Actor<Context=Context<A>>,
        M: MessageExt,
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

/// To permit extending the base app, a `OpaqueMessage` type can be used,
/// which specifies the type of the message with a string, and contains the
/// serialized inner bytes.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OpaqueMessage {
    pub id: String,
    pub inner: Vec<u8>,
}

impl Message for OpaqueMessage {
    type Result = OpaqueMessage;
}

impl MessageExt for OpaqueMessage {
    type Response = <Self as Message>::Result;

    const PATH: &'static str = "";
}



#[cfg(test)]
mod tests {
	use actix::{Actor, System};
	use actix_web::server;
	use failure::Error;
	use futures::Future;
	use url::Url;

	use std::sync::mpsc;
	use std::{time, thread};

	use crate::prelude::*;
	use crate::test_helpers::*;

	#[test]
	fn test_route() {
		init_logger();
		let mut sys = actix::System::new("test-sys");

		let handler = TestHandler::start_default();
		app::App::new()
			.route::<TestMessage, _>(handler, RouteType::Server)
			.make_current();

		let fut = app::send_in(TestMessage(42));
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.0, 42);

		let fut = app::send_local(TestMessage(42));
		let res = sys.block_on(fut);
		assert!(res.is_err());
	}

	#[test]
	fn test_service() {
		init_logger();
		let mut sys = actix::System::new("test-sys");

		let handler = TestHandler::default();
		app::App::new()
			.service(handler)
			.make_current();

		let fut = app::send_in(TestMessage(42));
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.0, 42);

		let fut = app::send_in(TestMessageEmpty);
		let _res = sys.block_on(fut).unwrap();
	}

	#[test]
	fn test_stringly_route() {
		init_logger();
		let mut sys = actix::System::new("test-sys");

		let handler = ("test", TestHandler::start_default());
		app::App::new()
			.route(handler, RouteType::Server)
			.make_current();

		let fut = app::send_in(crate::OpaqueMessage { id: "test".to_string(), inner: Vec::new()});
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.id, "test_response");

		let fut = app::send_in(TestMessage(42));
		let res = sys.block_on(fut);
		assert!(res.is_err());
	}

	#[test]
	fn test_stringly_service() {
		init_logger();
		let mut sys = actix::System::new("test-sys");

		let handler = TestHandler::default();
		app::App::new()
			.service(handler)
			.make_current();

		let fut = app::send_in(TestMessage(42));
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.0, 42);

		let fut = app::send_in(crate::OpaqueMessage { id: "test".to_string(), inner: Vec::new()});
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.id, "test_response");

		let fut = app::send_in(crate::OpaqueMessage { id: "other".to_string(), inner: Vec::new()});
		let res = sys.block_on(fut);
		assert!(res.is_err());
	}

	#[test]
	fn test_fut_service() {
		init_logger();
		let mut sys = actix::System::new("test-sys");

		let (p, c) = futures::sync::oneshot::channel::<()>();

		thread::spawn(|| {
			let t = time::Duration::from_millis(100);
			log::trace!("Future sleeping");
			thread::sleep(t);
			log::trace!("Future waking up");
		    let _ = p.send(());
		});
		let fut = c.map_err(Error::from).map(|_| {
			log::trace!("Create TestHandler");
			TestHandler::start_default()
		});
		app::App::new()
			.route::<TestMessage, _>(PendingRoute::new(fut), RouteType::Server)
			.make_current();

		let fut = app::send_in(TestMessage(42));
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.0, 42);

		let fut = app::send_local(TestMessage(42));
		let res = sys.block_on(fut);
		assert!(res.is_err());
	}

	#[test]
	fn test_http_service() {
	    init_logger();
	    let mut sys = System::new("test_client");
	    let (sender, receiver) = mpsc::sync_channel(1);
	    thread::spawn(move || {
		    let sys = System::new("test_server");
	        let addr = TestHandler::default();
	        let ad_app = app::App::new()
	            .service(addr);
	       	let app_fact = ad_app.http_server().clone();
	        ad_app.make_current();
	        let server = server::new(app_fact).bind("0.0.0.0:0").unwrap();
	        sender.send(format!("http://{}/{}", server.addrs()[0], TestMessage::PATH)).unwrap();
	        server.start();
	        sys.run();
	    });

	    thread::sleep(time::Duration::from_millis(100));

	    let url = receiver.recv().unwrap();
	    log::trace!("Test URL: {}", url);
	    let url = Url::parse(&url).unwrap();

	    let _service = app::App::new()
	    	.service(TestHandler::default())
	    	.route::<TestMessage, _>(app::no_client(), RouteType::Client)
	    	.route::<TestMessage, _>(url, RouteType::Upstream)
	        .make_current();

	    let res = sys.block_on(app::send(TestMessage(138)));
	    log::trace!("RPC result: {:?}", res);
	    let res = res.unwrap();
	    assert_eq!(res.0, 138);
	}

	#[test]
	fn test_rpc_service() {
	    init_logger();
	    let mut sys = System::new("test_client");
	    let (sender, receiver) = mpsc::sync_channel(1);
	    thread::spawn(move || {
		    let mut sys = System::new("test_server");
	        let addr = TestHandler::default();
	        let app = app::App::new()
	            .service(addr);
	       	let socket_addr = app.serve_local_http(None);
	        app.make_current();
	        sender.send(socket_addr).unwrap();
	        sys.run();
	        log::info!("End RPC server thread");
	    });

	    thread::sleep(time::Duration::from_millis(100));
	    let socket_addr = receiver.recv().unwrap();

	    let _service = app::App::new()
	    	.route::<TestMessage, _>(socket_addr, RouteType::Upstream)
	        .make_current();

	    let res = sys.block_on(app::send_out(TestMessage(69)));
	    log::trace!("RPC result: {:?}", res);
	    let res = res.unwrap();
	    assert_eq!(res.0, 69);
	}

	#[cfg(unix)]
	#[test]
	fn test_plugin() {
	    init_logger();
	    let mut sys = System::new("test_server");
        let addr = TestHandler::default();
        let plugin = crate::test_helpers::test_plugin();
        let mut app = app::App::new()
        				.plugin(plugin);
       	let socket_addr = app.serve_local_http(None);
        app.make_current();

        // Give the plugin time to spin up?
        thread::sleep(time::Duration::from_millis(100));

        let msg = crate::OpaqueMessage { id: "test".to_string(), inner: Vec::new()};
	    let _res = sys.block_on(app::send_out(msg)).unwrap();
	}
}

#[cfg(not(feature = "print_types"))]
#[macro_export]
macro_rules! get_type {
	($T:ty) => (
		::std::any::TypeId::of::<$T>()
	)
}
#[cfg(feature = "print_types")]
#[macro_export]
macro_rules! get_type {
	($T:ty) => (
		unsafe { ::std::intrinsics::type_name::<$T>() }
	)
}


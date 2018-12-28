#![cfg_attr(feature="print_types", feature(core_intrinsics))]

pub mod app;
pub mod http;
mod router;
pub mod service;

pub use self::app::{App, Routeable, RouteType};
pub use self::router::{PendingRoute, StringifiedMessage};

#[cfg(test)]
pub mod test_helpers;

use ::actix::dev::*;
use failure::Error;
use futures::Future;
use serde::{de::DeserializeOwned, Serialize};

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

impl<F, M> From<F> for SoarResponse<M>
    where
        M: SoarMessage, 
        F: 'static + Future<Item=M::Response, Error=Error>,
{
    fn from(other: F) -> Self {
        SoarResponse(Box::new(other))
    }
}

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


#[cfg(test)]
mod tests {
	use actix::{Actor, System};
	// use actix_web::server;
	use actix_web::test::TestServer;
	use failure::Error;
	use futures::Future;
	use url::Url;

	use std::sync::mpsc;
	use std::{time, thread};

	use crate::*;
	use crate::http::HttpSoarApp;
	use crate::app;
	use crate::test_helpers::*;
	// use crate::Pending;

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
		let res = sys.block_on(fut).unwrap();
	}

	#[test]
	fn test_stringly_route() {
		init_logger();
		let mut sys = actix::System::new("test-sys");

		let handler = ("test", TestHandler::start_default());
		app::App::new()
			.route(handler, RouteType::Server)
			.make_current();

		let fut = app::send_in(crate::StringifiedMessage { id: "test".to_string(), inner: Vec::new()});
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.unwrap().id, "test_response");

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

		let fut = app::send_in(crate::StringifiedMessage { id: "test".to_string(), inner: Vec::new()});
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.unwrap().id, "test_response");

		let fut = app::send_in(crate::StringifiedMessage { id: "other".to_string(), inner: Vec::new()});
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

	    std::thread::spawn(move || {
		    // actix_web::test::TestServer does some actix::System
		    // shenanigans. Easier to run everything inside the closure.
		    let server = TestServer::new(move |app| {
		        let addr = TestHandler::default();
		        let service = app::App::new()
		            .service(addr)
		            .make_current();
		        app.message::<TestMessage>("/test");
		    });
	        sender.send(server.url("/test")).unwrap();
	    });

	    let url = Url::parse(&receiver.recv().unwrap()).unwrap();

	    let _service = app::App::new()
	    	.service(TestHandler::default())
	    	.route::<TestMessage, _>(url, RouteType::Upstream)
	        .make_current();

	    let res = sys.block_on(app::send(TestMessage(138))).unwrap();
	    assert_eq!(res.0, 138);
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


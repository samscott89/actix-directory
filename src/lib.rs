#![cfg_attr(feature="print_types", feature(core_intrinsics))]

pub mod http;
pub mod service;
mod router;

pub use self::router::{PendingRoute, SoarMessage, SoarResponse};

#[cfg(test)]
pub mod test_helpers;

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

	use crate::http::HttpSoarApp;
	use crate::service;
	use crate::test_helpers::*;
	use crate::PendingRoute;

	#[test]
	fn test_service() {
		init_logger();
		let mut sys = actix::System::new("test-sys");

		let handler = TestHandler::start_default();
		let _service = service::Service::new()
			.add_local::<TestMessage, _>(handler)
			.run();

		let fut = service::send(TestMessage(42));
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.0, 42);
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
		let handler = PendingRoute::new(fut);
		let _service = service::Service::new()
			.add_local::<TestMessage, _>(handler)
			.run();

		let fut = service::send(TestMessage(42));
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.0, 42);
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
		        let addr = TestHandler::start_default();
		        let _service = service::Service::new()
		            .add_local::<TestMessage, _>(addr)
		            .run();
		        app.message::<TestMessage>("/test");
		    });
	        sender.send(server.url("/test")).unwrap();
	    });

	    let url = Url::parse(&receiver.recv().unwrap()).unwrap();

	    let _service = service::Service::new()
	    	.add_local::<TestMessage, _>(service::no_client())
	        .add_http_remote::<TestMessage>(url)
	        .run();

	    let res = sys.block_on(service::send(TestMessage(138))).unwrap();
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


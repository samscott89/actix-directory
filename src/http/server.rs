use ::actix::dev::*;
use actix_web::{App, AsyncResponder, HttpMessage, HttpRequest, HttpResponse};
use failure::Error;
use futures::{future, Future};
use log::*;

use crate::service;
use crate::service::SoarMessage;

/// An `HttpSoarApp` is ultimately used to extend an `actix_web::App`,
/// by adding the method `message`.
pub trait HttpSoarApp {

	/// Register the path `path` as able to respond to requests for the
	/// message type `M`.
	/// Since this will use the `Addr<Service>` in the `App` state,
	/// this handler must have been previously registered.
	fn message<M>(self, path: &str) -> Self
		where
		    M: SoarMessage;
}

impl HttpSoarApp for App {
	fn message<M>(self, path: &str) -> Self
		where
		    M: SoarMessage,
	{
		self.resource(path, |r| { r.f(|r| handle_request::<M>(r)) })
	}
}

#[cfg(test)]
impl HttpSoarApp for &mut actix_web::test::TestApp {
	fn message<M>(self, path: &str) -> Self
		where
		    M: SoarMessage
	{
		trace!("TEST: Handle message");
		self.resource(path, |r| { r.f(|r| handle_request::<M>(r)) })
	}
}


/// Simple wrapper function. Deserialize request, and serialize the output.
pub fn handle_request<M: 'static>(
    req: &HttpRequest,
) -> impl actix_web::Responder
	where
	    M: SoarMessage
{
    req.body().map_err(Error::from)
    	.and_then(|body| {
    		bincode::deserialize(&body).map_err(Error::from)
    	})
        .and_then(move |req: M| service::send(req).map_err(Error::from))
        .and_then(|resp| future::result(bincode::serialize(&resp)).map_err(Error::from))
        .map(|resp| {
            trace!("Handled request successfully");
            HttpResponse::Ok().body(resp)
        })
        .responder()
}

#[cfg(test)]
mod tests {
	use actix_web::test::TestServer;

	use super::*;
	use super::HttpSoarApp;
	use crate::test_helpers::*;

	#[test]
	fn test_http_server() {
		init_logger();
		let mut sys = System::new("test-sys");

		let server = TestServer::new(|app| {
			let addr = TestHandler::start_default();
			service::add_route::<TestMessage, _>(addr);
			app.message::<TestMessage>("/test");
		});

		let msg = bincode::serialize(&TestMessage(12)).unwrap();
		let req = server.post().uri(server.url("/test")).body(msg).unwrap();
		trace!("{:?}", req);
		let response = sys.block_on(req.send()).unwrap();
		assert!(response.status().is_success());
		assert_eq!(
			bincode::deserialize::<TestResponse>(
				&response.body().wait().unwrap()
			).unwrap(),
			TestResponse(12)
		);
	}
}
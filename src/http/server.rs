use ::actix::dev::*;
use actix_web::{App, AsyncResponder, HttpMessage, HttpRequest, HttpResponse};
use failure::Error;
use futures::Future;
use log::*;

use crate::*;

pub trait HttpSoarApp {
	fn message<M>(self, path: &str) -> Self
		where
		    M: SoarMessage;
}

impl HttpSoarApp for App<Addr<Service>> {
	fn message<M>(self, path: &str) -> Self
		where
		    M: SoarMessage,
	{
		self.resource(path, |r| { r.f(|r| handle_request::<M>(r)) })
	}
}

#[cfg(test)]
impl HttpSoarApp for &mut actix_web::test::TestApp<Addr<Service>> {
	fn message<M>(self, path: &str) -> Self
		where
		    M: SoarMessage
	{
		trace!("TEST: Handle message");
		self.resource(path, |r| { r.f(|r| handle_request::<M>(r)) })
	}
}


pub fn handle_request<M: 'static>(
    req: &HttpRequest<Addr<Service>>,
) -> impl actix_web::Responder
	where
	    M: SoarMessage
{
    let service = req.state().clone();
    req.json::<M>().map_err(Error::from)
        .and_then(move |req| service.send(req).map_err(Error::from))
        .map(|resp| {
            trace!("Handled request successfully");
            HttpResponse::Ok().json(resp)
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
		let service = start_service(|| {
			let mut service = Service::new("test_http_server");
			service.add_handler(TestHandler);
			service
		});

		let server = TestServer::build_with_state(move || service.clone())
							.start(|app| {
								app.message::<TestMessage>("/test");
							});

		let req = server.post().uri(server.url("/test")).json(TestMessage(12)).unwrap();
		trace!("{:?}", req);
		let response = sys.block_on(req.send()).unwrap();
		assert!(response.status().is_success());
		assert_eq!(response.json::<TestResponse>().wait().unwrap(), TestResponse(12));

	}
}
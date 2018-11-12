use ::actix::dev::*;
use actix_web::{server, App, AsyncResponder, HttpMessage, HttpRequest, HttpResponse};
use failure::Error;
use futures::{future, Future};
use log::*;
use query_interface::{HasInterface, Object};
use serde::{de::DeserializeOwned, Serialize};

use super::*;

pub trait HttpSoarApp {
	fn message<M, I, E>(self, path: &str) -> Self
		where
		    M: Message<Result=Result<I, E>> + 'static + DeserializeOwned + Send,
		    I: 'static + Send + Serialize,
		    E: 'static + Send + Serialize;
}

impl HttpSoarApp for App<Addr<Service>> {
	fn message<M, I, E>(self, path: &str) -> Self
		where
		    M: Message<Result=Result<I, E>> + 'static + DeserializeOwned + Send,
		    I: 'static + Send + Serialize,
		    E: 'static + Send + Serialize,
	{
		self.resource(path, |r| { r.f(|r| handle_request::<M, I, E>(r)) })
	}
}

#[cfg(test)]
impl HttpSoarApp for &mut actix_web::test::TestApp<Addr<Service>> {
	fn message<M, I, E>(self, path: &str) -> Self
		where
		    M: Message<Result=Result<I, E>> + 'static + DeserializeOwned + Send,
		    I: 'static + Send + Serialize,
		    E: 'static + Send + Serialize,
	{
		trace!("TEST: Handle message");
		self.resource(path, |r| { r.f(|r| handle_request::<M, I, E>(r)) })
	}
}


pub fn handle_request<M: 'static, I, E>(
    req: &HttpRequest<Addr<Service>>,
) -> impl actix_web::Responder
	where
	    M: Message<Result=Result<I, E>> + 'static + DeserializeOwned + Send,
	    I: 'static + Send + Serialize,
	    E: 'static + Send + Serialize,
{
    let service = req.state().clone();
    req.json::<M>().map_err(Error::from)
        .and_then(move |req| service.send(req).map_err(Error::from))
        .and_then(|resp| {
            trace!("Handled request successfully");
            future::ok(HttpResponse::Ok().json(resp))
        })
        .responder()
}

#[cfg(test)]
mod tests {
	use actix_web::test::TestServer;
	use super::*;
	use crate::test_helpers::*;

	#[test]
	fn test_http_server() {
		init_logger();
		let mut sys = System::new("test-sys");
		let service = start_service(|| {
			let mut service = Service::new();
			service.add_handler(TestHandler);
			service
		});

		let mut server = TestServer::build_with_state(move || service.clone())
							.start(|app| {
								app.message::<TestMessage, TestResponse, ()>("/test");
							});

		let req = server.post().uri(server.url("/test")).json(TestMessage(12)).unwrap();
		trace!("{:?}", req);
		let response = sys.block_on(req.send()).unwrap();
		assert!(response.status().is_success());
		assert_eq!(response.json::<Result<TestResponse, ()>>().wait().unwrap(), Ok(TestResponse(12)));
	}
}
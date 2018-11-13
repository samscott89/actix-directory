use ::actix::dev::*;
use failure::{Error, Fail};
use futures::{future, Future};
// use failure_derive::Fail;
use log::*;
use query_interface::{HasInterface, Object};
use serde::{de::DeserializeOwned, Serialize};
use serde_derive::{Deserialize, Serialize};

use std::collections::HashMap;
use std::any::TypeId;

mod channel;
mod http;

#[cfg(test)]
pub mod test_helpers;

pub trait SoarMessage:
	Message<Result=<Self as SoarMessage>::Response>
	+ 'static + Send + DeserializeOwned + Serialize
{
	type Response: 'static + Send + DeserializeOwned + Serialize;
}

pub struct SoarResponse<M: SoarMessage>(pub Box<Future<Item=M::Response, Error=Error>>);

impl<E, F, M> From<Result<F, E>> for SoarResponse<M>
	where E: Fail,
	      F: 'static + Future<Item=M::Response, Error=Error>,
		  M: SoarMessage,
{
	fn from(other: Result<F, E>) -> Self {
		match other {
			Ok(fut) => SoarResponse(Box::new(fut)),
			Err(e) => SoarResponse(Box::new(future::err(Error::from(e)))),
		}
	}
}

impl<M> MessageResponse<Service, M> for SoarResponse<M>
where
    M: SoarMessage,
{
    fn handle<R: ResponseChannel<M>>(self, _ctxt: &mut Context<Service>, tx: Option<R>) {
		Arbiter::spawn(self.0.and_then(move |res| {
			if let Some(tx) = tx {
				tx.send(res)
			}
			Ok(())
		}).map_err(|_| ()));
	}
}

/// A `RequestHandler` is responsible for taking 
pub trait RequestHandler<M>: Send
	where M: SoarMessage
{
	fn handle_request(&self, msg: M, service: &Service) -> Box<Future<Item=M::Response, Error=Error>>;
}

pub struct Service {
	name: String,
	handlers: HashMap<TypeId, Box<Object>>,
}

impl Actor for Service {
	type Context = actix::Context<Self>;

	fn started(&mut self, _ctx: &mut Self::Context) {
		trace!("Started service {}", self.name);
	}

	fn stopped(&mut self, _ctx: &mut Self::Context) {
		trace!("Stopped service {}", self.name);
	}
}

impl Service {
	pub fn new(name: &str) -> Self {
		Service {
			name: name.to_string(),
			handlers: HashMap::new(),
		}
	}
	pub fn add_handler<M, H>(&mut self, handler: H)
		where M: SoarMessage,
		      H: Object + HasInterface<RequestHandler<M>> + 'static
	{
		trace!("Store handler for message type: {:?}", TypeId::of::<M>());
		let handler = Box::new(handler) as Box<Object>;
		self.handlers.insert(TypeId::of::<M>(), handler);
	}

	fn get_handler<M>(&self) -> Option<&RequestHandler<M>>
		where  M: SoarMessage,
	{
		trace!("Lookup request handler");
		self.handlers.get(&TypeId::of::<M>())
					 .and_then(|h| {
					 	trace!("Found entry, getting object as RequestHandler");
					 	h.query_ref::<RequestHandler<M>>()
					 })
	}
}

#[derive(Default, Deserialize, Serialize, Fail, Debug)]
#[fail(display = "no handler found")]
pub struct ServiceError {}

impl<M> Handler<M> for Service
	where  M: SoarMessage
{
	type Result = SoarResponse<M>;

	fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
		trace!("Get handler for message type: {:?}", TypeId::of::<M>());
		let handler = self.get_handler::<M>().ok_or_else(ServiceError::default);
		SoarResponse::from(handler.map(|h| h.handle_request(msg, &self)))
	}
}


#[cfg(test)]
mod tests {
	use super::*;
	use crate::test_helpers::*;

	#[test]
	fn test_handler() {
		let mut sys = System::new("test-sys");
		let mut service = Service::new("test");
		service.add_handler(TestHandler);

		let addr = service.start();

		let fut = addr.send(TestMessage(42));
		let res = sys.block_on(fut).unwrap();
		assert_eq!(res.0, 42);
	}
}

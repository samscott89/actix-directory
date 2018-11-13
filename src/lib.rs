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

pub mod channel;
pub mod http;

#[cfg(test)]
pub mod test_helpers;

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

/// Convenience type for return type of `RequestHandler<M>`.
pub type RespFuture<M> = Box<Future<Item=<M as SoarMessage>::Response, Error=Error>>;

/// A `RequestHandler` is effectively a simplified version of the `actix::Handler` trait.
/// It does not need to know about the context, and instead provides a reference to the 
/// `Service` it is running on, to allow arbitrary other queries to be chained.
///
/// An additional error `Error` is added to the return type of the `Message::Response`,
/// since the meta-service-handler `Service` can also fail.
pub trait RequestHandler<M>: Send
	where M: SoarMessage
{
	fn handle_request(&self, msg: M, service: &Service) -> RespFuture<M>;
}


/// The core of `soar` is the `Service` struct. 
/// It maintains a map from `M: Message`s (in the form of the `TypeId`), to `RequestHandler<M>`
/// implementations.
/// Only a single handler can be added for each `Message` type.
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
	/// Create a new `Service` with the given name. 
	/// The name is purely used for logging/debugging purposes and is not 
	/// guaranteed/needed to be unique.
	pub fn new(name: &str) -> Self {
		Service {
			name: name.to_string(),
			handlers: HashMap::new(),
		}
	}

	/// Add the handler to the `Service`.
	pub fn add_handler<M, H>(&mut self, handler: H)
		where M: SoarMessage,
		      H: Object + HasInterface<RequestHandler<M>> + 'static
	{
		trace!("Store handler for message type: {:?}", TypeId::of::<M>());
		let handler = Box::new(handler) as Box<Object>;
		self.handlers.insert(TypeId::of::<M>(), handler);
	}

	/// Get the handler identified by the generic type parameter `M`.
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
/// `Service` fails when there is no known handler for a given message.
pub struct ServiceError {}

impl<M> Handler<M> for Service
	where  M: SoarMessage
{
	type Result = SoarResponse<M>;

	fn handle(&mut self, msg: M, _ctxt: &mut Context<Self>) -> Self::Result {
		trace!("Get handler for message type: {:?}", TypeId::of::<M>());
		let handler = self.get_handler::<M>().ok_or_else(ServiceError::default);
		let resp = handler.map(|h| h.handle_request(msg, &self));
		match resp {
			Ok(fut) => SoarResponse(Box::new(fut)),
			Err(e) => SoarResponse(Box::new(future::err(Error::from(e)))),
		}
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

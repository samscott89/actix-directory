use ::actix::dev::*;
use log::*;
use query_interface::{HasInterface, Object};

use std::collections::HashMap;
use std::any::TypeId;

mod http;

#[cfg(test)]
pub mod test_helpers;

/// A `RequestHandler` is responsible for taking 
pub trait RequestHandler<M, I, E>: Send
	where M: Message<Result = Result<I, E>>,
		  I: 'static, E: 'static
{
	fn handle_request(&self, msg: M, service: &Service) -> ResponseFuture<I, E>;
}

pub struct Service {
	handlers: HashMap<TypeId, Box<Object>>,
}

impl Actor for Service {
	type Context = actix::Context<Self>;

	fn started(&mut self, _ctx: &mut Self::Context) {
		trace!("Started Service");
	}

	fn stopped(&mut self, ctx: &mut Self::Context) {
		trace!("Stopped Service");
	}
}

impl Service {
	pub fn new() -> Self {
		Service {
			handlers: HashMap::new(),
		}
	}
	pub fn add_handler<M, I: 'static, E: 'static, H>(&mut self, handler: H)
		where M: Message<Result = Result<I, E>> + 'static,
		      H: Object + HasInterface<RequestHandler<M, I, E>> + 'static
	{
		trace!("Store handler for message type: {:?}", TypeId::of::<M>());
		let handler = Box::new(handler) as Box<Object>;
		self.handlers.insert(TypeId::of::<M>(), handler);
	}

	fn get_handler<M, I, E>(&self) -> Option<&RequestHandler<M, I, E>>
		where  M: Message<Result=Result<I, E>> + 'static,
			   I: 'static + Send,
			   E: 'static + Send,
	{
		trace!("Lookup request handler");
		self.handlers.get(&TypeId::of::<M>())
					 .and_then(|h| {
					 	trace!("Found entry, getting object as RequestHandler");
					 	h.query_ref::<RequestHandler<M, I, E>>()
					 })
	}
}

impl<M, I, E> Handler<M> for Service
	where  M: Message<Result=Result<I, E>> + 'static,
		   I: 'static + Send,
		   E: 'static + Send,
{
	type Result = ResponseFuture<I, E>;

	fn handle(&mut self, msg: M, ctxt: &mut Context<Self>) -> Self::Result {
		trace!("Get handler for message type: {:?}", TypeId::of::<M>());
		let handler = self.get_handler::<M, I, E>().expect("message handler missing");
		handler.handle_request(msg, &self)
	}
}


#[cfg(test)]
mod tests {
	use futures::{future, Future};
	use std::env;

	use super::*;
	use crate::test_helpers::*;


	#[test]
	fn name() {
		let mut sys = System::new("test-sys");
		let mut service = Service::new();
		service.add_handler(TestHandler);

		let addr = service.start();

		let fut = addr.send(TestMessage(42));
		let res = sys.block_on(fut).unwrap().unwrap();
		assert_eq!(res.0, 42);
	}
}
